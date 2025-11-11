"""
Tests for llm_adapter module
"""

import sys
import os
import tempfile
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent / '.claude/hooks'))

import pytest
from llm_adapter import LLMAdapter, CostTracker, create_llm_adapter


class TestCostTracker:
    """Test cost tracking functionality."""

    def test_cost_tracker_init(self):
        """Test cost tracker initialization."""
        with tempfile.TemporaryDirectory() as tmpdir:
            usage_file = os.path.join(tmpdir, 'usage.json')
            tracker = CostTracker(usage_file)

            assert tracker.usage_file == Path(usage_file)

    def test_load_usage_creates_default(self):
        """Test that load_usage creates default data if file doesn't exist."""
        with tempfile.TemporaryDirectory() as tmpdir:
            usage_file = os.path.join(tmpdir, 'usage.json')
            tracker = CostTracker(usage_file)

            usage = tracker.load_usage()

            assert 'date' in usage
            assert usage['cost_usd'] == 0.0
            assert usage['calls'] == 0
            assert 'tokens' in usage

    def test_record_usage(self):
        """Test recording usage data."""
        with tempfile.TemporaryDirectory() as tmpdir:
            usage_file = os.path.join(tmpdir, 'usage.json')
            tracker = CostTracker(usage_file)

            # Record some usage
            tracker.record_usage(
                model='test-model',
                input_tokens=100,
                output_tokens=50,
                cost_usd=0.001
            )

            # Load and verify
            usage = tracker.load_usage()

            assert usage['calls'] == 1
            assert usage['cost_usd'] == 0.001
            assert usage['tokens']['input'] == 100
            assert usage['tokens']['output'] == 50
            assert usage['tokens']['total'] == 150
            assert 'test-model' in usage['models']

    def test_check_budget_under_limit(self):
        """Test budget check when under limit."""
        with tempfile.TemporaryDirectory() as tmpdir:
            usage_file = os.path.join(tmpdir, 'usage.json')
            tracker = CostTracker(usage_file)

            # Record small usage
            tracker.record_usage('test', 10, 10, 0.001)

            # Check budget
            assert tracker.check_budget(0.10) is True

    def test_check_budget_over_limit(self):
        """Test budget check when over limit."""
        with tempfile.TemporaryDirectory() as tmpdir:
            usage_file = os.path.join(tmpdir, 'usage.json')
            tracker = CostTracker(usage_file)

            # Record large usage
            tracker.record_usage('test', 1000, 1000, 0.15)

            # Check budget
            assert tracker.check_budget(0.10) is False


class TestLLMAdapter:
    """Test LLM adapter functionality."""

    @pytest.fixture
    def test_config(self):
        """Provide test configuration."""
        return {
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
                'usage_file': tempfile.mktemp(suffix='.json')
            }
        }

    def test_adapter_init(self, test_config):
        """Test adapter initialization."""
        adapter = LLMAdapter(test_config)

        assert adapter.models == test_config['models']
        assert adapter.parameters == test_config['parameters']
        assert adapter.cost_control == test_config['cost_control']

    def test_get_model_priority(self, test_config):
        """Test model priority list."""
        adapter = LLMAdapter(test_config)

        priority = adapter._get_model_priority()

        assert len(priority) == 3
        assert priority[0] == 'gemini/gemini-2.0-flash-exp'
        assert priority[1] == 'claude-3-haiku-20240307'
        assert priority[2] == 'ollama/llama3.2'

    def test_estimate_cost_gemini(self, test_config):
        """Test cost estimation for Gemini model."""
        adapter = LLMAdapter(test_config)

        cost = adapter._estimate_cost('gemini/gemini-2.0-flash-exp', 1000, 500)

        # Gemini Flash: $0.075 per 1M input, $0.30 per 1M output
        # 1000 input tokens = $0.000075
        # 500 output tokens = $0.00015
        expected = 0.000075 + 0.00015
        assert abs(cost - expected) < 0.000001

    def test_estimate_cost_claude(self, test_config):
        """Test cost estimation for Claude model."""
        adapter = LLMAdapter(test_config)

        cost = adapter._estimate_cost('claude-3-haiku-20240307', 1000, 500)

        # Claude Haiku: $0.25 per 1M input, $1.25 per 1M output
        # 1000 input tokens = $0.00025
        # 500 output tokens = $0.000625
        expected = 0.00025 + 0.000625
        assert abs(cost - expected) < 0.000001

    def test_estimate_cost_ollama(self, test_config):
        """Test cost estimation for Ollama (should be 0)."""
        adapter = LLMAdapter(test_config)

        cost = adapter._estimate_cost('ollama/llama3.2', 1000, 500)

        assert cost == 0.0

    def test_factory_function(self, test_config):
        """Test factory function creates adapter correctly."""
        adapter = create_llm_adapter(test_config)

        assert isinstance(adapter, LLMAdapter)


@pytest.mark.skipif(not os.getenv('GEMINI_API_KEY'), reason="Requires GEMINI_API_KEY")
class TestLLMIntegration:
    """Integration tests requiring actual API keys."""

    @pytest.fixture
    def test_config(self):
        """Provide test configuration with real API key."""
        return {
            'models': {
                'primary': 'gemini/gemini-2.0-flash-exp'
            },
            'api_keys': {
                'gemini': '${GEMINI_API_KEY}'
            },
            'parameters': {
                'max_tokens': 50,
                'temperature': 0.3,
                'timeout': 10
            },
            'cost_control': {
                'daily_limit_usd': 0.10,
                'usage_tracking': True,
                'usage_file': tempfile.mktemp(suffix='.json')
            }
        }

    def test_generate_summary_with_api(self, test_config):
        """Test actual summary generation with API."""
        adapter = LLMAdapter(test_config)

        context = "This is a test message to verify that the LLM adapter works correctly."
        summary = adapter.generate_summary(context, max_length=50)

        assert summary is not None
        assert len(summary) <= 50
        assert isinstance(summary, str)


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
