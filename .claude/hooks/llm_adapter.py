"""
LiteLLM Multi-Model Adapter

Provides unified interface for multiple LLM providers with fallback support.
"""

import os
import logging
import json
from typing import Optional, Dict, Any, List
from datetime import datetime
from pathlib import Path

import litellm

logger = logging.getLogger(__name__)

# Suppress LiteLLM verbose logging
litellm.suppress_debug_info = True


class CostTracker:
    """Track LLM usage and costs."""

    def __init__(self, usage_file: str = "~/.claude/voice-usage.json"):
        self.usage_file = Path(usage_file).expanduser()
        self.usage_file.parent.mkdir(parents=True, exist_ok=True)

    def load_usage(self) -> Dict[str, Any]:
        """Load usage data from file."""
        if not self.usage_file.exists():
            return {
                'date': datetime.now().date().isoformat(),
                'cost_usd': 0.0,
                'calls': 0,
                'tokens': {'input': 0, 'output': 0, 'total': 0},
                'models': {}
            }

        try:
            with open(self.usage_file, 'r') as f:
                return json.load(f)
        except Exception as e:
            logger.error(f"Failed to load usage file: {e}")
            return self.load_usage()  # Return fresh data

    def save_usage(self, usage: Dict[str, Any]):
        """Save usage data to file."""
        try:
            with open(self.usage_file, 'w') as f:
                json.dump(usage, f, indent=2)
        except Exception as e:
            logger.error(f"Failed to save usage file: {e}")

    def check_budget(self, daily_limit_usd: float) -> bool:
        """
        Check if daily budget has been exceeded.

        Args:
            daily_limit_usd: Daily spending limit in USD

        Returns:
            True if under budget, False if exceeded
        """
        usage = self.load_usage()
        today = datetime.now().date().isoformat()

        # Reset if new day
        if usage.get('date') != today:
            usage = {
                'date': today,
                'cost_usd': 0.0,
                'calls': 0,
                'tokens': {'input': 0, 'output': 0, 'total': 0},
                'models': {}
            }
            self.save_usage(usage)

        return usage['cost_usd'] < daily_limit_usd

    def record_usage(
        self,
        model: str,
        input_tokens: int,
        output_tokens: int,
        cost_usd: float
    ):
        """
        Record usage for a single API call.

        Args:
            model: Model name used
            input_tokens: Number of input tokens
            output_tokens: Number of output tokens
            cost_usd: Cost in USD
        """
        usage = self.load_usage()
        today = datetime.now().date().isoformat()

        # Reset if new day
        if usage.get('date') != today:
            usage = {
                'date': today,
                'cost_usd': 0.0,
                'calls': 0,
                'tokens': {'input': 0, 'output': 0, 'total': 0},
                'models': {}
            }

        # Update totals
        usage['calls'] += 1
        usage['cost_usd'] += cost_usd
        usage['tokens']['input'] += input_tokens
        usage['tokens']['output'] += output_tokens
        usage['tokens']['total'] += (input_tokens + output_tokens)

        # Update per-model stats
        if model not in usage['models']:
            usage['models'][model] = {
                'calls': 0,
                'cost_usd': 0.0,
                'tokens': {'input': 0, 'output': 0, 'total': 0}
            }

        usage['models'][model]['calls'] += 1
        usage['models'][model]['cost_usd'] += cost_usd
        usage['models'][model]['tokens']['input'] += input_tokens
        usage['models'][model]['tokens']['output'] += output_tokens
        usage['models'][model]['tokens']['total'] += (input_tokens + output_tokens)

        self.save_usage(usage)
        logger.info(f"Recorded usage: {model}, cost=${cost_usd:.6f}, tokens={input_tokens + output_tokens}")


class LLMAdapter:
    """
    Multi-model LLM adapter using LiteLLM.

    Supports:
    - Multiple providers (Gemini, Claude, OpenAI, Ollama)
    - Automatic fallback on failure
    - Cost tracking and budget limits
    - Token usage monitoring
    """

    def __init__(self, config: Dict[str, Any]):
        """
        Initialize LLM adapter with configuration.

        Args:
            config: LLM configuration from voice_config.json
        """
        self.config = config
        self.models = config.get('models', {})
        self.parameters = config.get('parameters', {})
        self.cost_control = config.get('cost_control', {})

        # Set up API keys from environment
        self._setup_api_keys()

        # Initialize cost tracker
        usage_file = self.cost_control.get('usage_file', '~/.claude/voice-usage.json')
        self.cost_tracker = CostTracker(usage_file) if self.cost_control.get('usage_tracking') else None

    def _setup_api_keys(self):
        """Load API keys from environment variables."""
        api_keys = self.config.get('api_keys', {})

        for provider, env_var_template in api_keys.items():
            # Extract env var name from template (e.g., "${GEMINI_API_KEY}" -> "GEMINI_API_KEY")
            if isinstance(env_var_template, str) and env_var_template.startswith('${') and env_var_template.endswith('}'):
                env_var = env_var_template[2:-1]
                value = os.getenv(env_var)

                if not value:
                    logger.warning(f"API key for {provider} not found in environment: {env_var}")

    def _get_model_priority(self) -> List[str]:
        """
        Get list of models in priority order.

        Returns:
            List of model names to try in order
        """
        priority = []

        # Add primary model
        if 'primary' in self.models and self.models['primary']:
            priority.append(self.models['primary'])

        # Add fallback model
        if 'fallback' in self.models and self.models['fallback']:
            priority.append(self.models['fallback'])

        # Add local model as last resort
        if 'local' in self.models and self.models['local']:
            priority.append(self.models['local'])

        return priority

    def generate_summary(self, context: str, max_length: int = 50) -> Optional[str]:
        """
        Generate summary using LLM with automatic fallback.

        Args:
            context: Context text to summarize
            max_length: Maximum length of summary in characters

        Returns:
            Generated summary or None if all models fail
        """
        # Check budget first
        daily_limit = self.cost_control.get('daily_limit_usd', 0.10)
        if self.cost_tracker and not self.cost_tracker.check_budget(daily_limit):
            logger.warning(f"Daily budget limit ${daily_limit} exceeded")
            return None

        models_to_try = self._get_model_priority()
        timeout = self.parameters.get('timeout', 10)
        max_tokens = self.parameters.get('max_tokens', 100)
        temperature = self.parameters.get('temperature', 0.3)

        # Build prompt
        prompt = f"Summarize the following in Traditional Chinese, max {max_length} characters:\n\n{context[:2000]}"

        for model in models_to_try:
            try:
                logger.info(f"Trying model: {model}")

                response = litellm.completion(
                    model=model,
                    messages=[{"role": "user", "content": prompt}],
                    max_tokens=max_tokens,
                    temperature=temperature,
                    timeout=timeout
                )

                # Extract summary
                summary = response.choices[0].message.content.strip()

                # Record usage
                if self.cost_tracker:
                    usage = response.usage
                    # LiteLLM doesn't always provide cost, estimate it
                    cost_usd = self._estimate_cost(model, usage.prompt_tokens, usage.completion_tokens)

                    self.cost_tracker.record_usage(
                        model=model,
                        input_tokens=usage.prompt_tokens,
                        output_tokens=usage.completion_tokens,
                        cost_usd=cost_usd
                    )

                logger.info(f"Successfully generated summary with {model}")
                return summary[:max_length]

            except Exception as e:
                logger.warning(f"Model {model} failed: {e}")
                continue

        # All models failed
        logger.error("All LLM models failed to generate summary")
        return None

    def _estimate_cost(self, model: str, input_tokens: int, output_tokens: int) -> float:
        """
        Estimate cost based on model and token usage.

        Args:
            model: Model name
            input_tokens: Number of input tokens
            output_tokens: Number of output tokens

        Returns:
            Estimated cost in USD
        """
        # Rough cost estimates (per 1K tokens)
        cost_map = {
            'gemini': {'input': 0.000075, 'output': 0.00030},  # Gemini Flash 2.0
            'claude': {'input': 0.00025, 'output': 0.00125},   # Claude Haiku
            'gpt-4o-mini': {'input': 0.00015, 'output': 0.00060},  # GPT-4o mini
            'ollama': {'input': 0.0, 'output': 0.0}  # Local is free
        }

        # Find matching pricing
        pricing = None
        for key in cost_map:
            if key in model.lower():
                pricing = cost_map[key]
                break

        if not pricing:
            # Default to Claude pricing
            pricing = cost_map['claude']

        input_cost = (input_tokens / 1000) * pricing['input']
        output_cost = (output_tokens / 1000) * pricing['output']

        return input_cost + output_cost


def create_llm_adapter(config: Dict[str, Any]) -> LLMAdapter:
    """
    Factory function to create LLMAdapter instance.

    Args:
        config: LLM configuration dictionary

    Returns:
        Configured LLMAdapter instance
    """
    return LLMAdapter(config)


if __name__ == "__main__":
    # Quick test
    logging.basicConfig(level=logging.INFO)

    # Test configuration
    test_config = {
        'models': {
            'primary': 'gemini/gemini-2.0-flash-exp',
            'fallback': 'claude-3-haiku-20240307',
            'local': 'ollama/llama3.2'
        },
        'api_keys': {
            'gemini': '${GEMINI_API_KEY}',
            'anthropic': '${ANTHROPIC_API_KEY}'
        },
        'parameters': {
            'max_tokens': 100,
            'temperature': 0.3,
            'timeout': 10
        },
        'cost_control': {
            'daily_limit_usd': 0.10,
            'usage_tracking': True,
            'usage_file': '~/.claude/voice-usage.json'
        }
    }

    adapter = LLMAdapter(test_config)

    print(f"Model priority: {adapter._get_model_priority()}")
    print(f"Budget OK: {adapter.cost_tracker.check_budget(0.10) if adapter.cost_tracker else 'N/A'}")

    # Test summary generation (requires API key)
    # summary = adapter.generate_summary("Test context for summarization", max_length=50)
    # print(f"Summary: {summary}")
