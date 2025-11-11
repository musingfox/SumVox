#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Claude Code Voice Notification Hook

Main entry point for voice notifications on Claude Code task completion.
This script is invoked by Claude Code's Stop hook mechanism.

Hook Mechanism:
- Triggered when Claude Code stops execution
- Receives JSON input via stdin with execution context
- Generates voice notification with task summary

Usage:
    Add to ~/.claude/settings.json:
    {
      "hooks": {
        "stop": ["/path/to/voice_notification.py"]
      }
    }

Input Format (JSON via stdin):
    {
      "output": "Raw execution output text",
      "duration": 12.5,
      "exit_code": 0,
      "timestamp": "2025-11-11T14:00:00Z"
    }
"""

import sys
import json
import logging
from pathlib import Path
from typing import Dict, Any, Optional

# Add hooks directory to path for imports
hooks_dir = Path(__file__).parent
sys.path.insert(0, str(hooks_dir))

from voice_config import load_config
from llm_adapter import create_llm_adapter
from summarizer import create_summarizer
from voice_engine import VoiceEngine

# Setup logging
def setup_logging(config: Dict[str, Any]) -> logging.Logger:
    """
    Configure logging based on config.

    Args:
        config: Logging configuration

    Returns:
        Configured logger instance
    """
    log_config = config.get('logging', {})

    if not log_config.get('enabled', True):
        logging.disable(logging.CRITICAL)
        return logging.getLogger(__name__)

    log_file = Path(log_config.get('log_file', '~/.claude/logs/voice-notifications.log')).expanduser()
    log_file.parent.mkdir(parents=True, exist_ok=True)

    log_level = getattr(logging, log_config.get('log_level', 'INFO'))

    # Configure logging
    logging.basicConfig(
        level=log_level,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
        handlers=[
            logging.FileHandler(log_file),
            logging.StreamHandler(sys.stderr)  # Also log to stderr
        ]
    )

    logger = logging.getLogger(__name__)
    logger.info("Voice notification hook started")

    return logger


class VoiceNotificationHook:
    """
    Main hook orchestrator.

    Coordinates LLM adapter, summarizer, and voice engine to generate
    voice notifications from Claude Code execution output.
    """

    def __init__(self, config: Dict[str, Any]):
        """
        Initialize hook with configuration.

        Args:
            config: Full configuration from voice_config.json
        """
        self.config = config
        self.logger = setup_logging(config)

        # Initialize components
        try:
            self.llm_adapter = create_llm_adapter(config['llm'])
            self.summarizer = create_summarizer(self.llm_adapter, config['summarization'])
            self.voice_engine = VoiceEngine(config['voice'])

            self.logger.info("All components initialized successfully")
        except Exception as e:
            self.logger.error(f"Failed to initialize components: {e}")
            raise

        self.triggers = config.get('triggers', {})
        self.advanced = config.get('advanced', {})

    def should_trigger(self, hook_input: Dict[str, Any]) -> bool:
        """
        Determine if notification should be triggered based on configuration.

        Args:
            hook_input: Input data from Claude Code hook

        Returns:
            True if notification should be triggered
        """
        # Check if enabled
        if not self.config.get('enabled', True):
            self.logger.info("Voice notifications disabled in config")
            return False

        # Check minimum duration
        min_duration = self.triggers.get('min_duration_seconds', 0)
        duration = hook_input.get('duration', 0)

        if duration < min_duration:
            self.logger.info(f"Duration {duration}s below minimum {min_duration}s, skipping")
            return False

        # Check exit code for errors
        exit_code = hook_input.get('exit_code', 0)
        output = hook_input.get('output', '')

        # Trigger on error if configured
        if exit_code != 0 or self._contains_error(output):
            if self.triggers.get('on_error', True):
                self.logger.info("Triggering on error")
                return True
            else:
                self.logger.info("Error detected but on_error=False, skipping")
                return False

        # Trigger on completion if configured
        if self.triggers.get('on_completion', True):
            self.logger.info("Triggering on completion")
            return True

        return False

    def _contains_error(self, output: str) -> bool:
        """
        Check if output contains error keywords.

        Args:
            output: Execution output text

        Returns:
            True if error keywords found
        """
        error_keywords = self.triggers.get('error_keywords', [])
        output_lower = output.lower()

        for keyword in error_keywords:
            if keyword.lower() in output_lower:
                return True

        return False

    def process_hook_input(self, hook_input: Dict[str, Any]) -> Optional[str]:
        """
        Process hook input and generate voice notification.

        Args:
            hook_input: Input data from Claude Code

        Returns:
            Summary text that was spoken, or None if skipped
        """
        import time

        pipeline_start = time.time()

        self.logger.info(f"Processing hook input: duration={hook_input.get('duration')}s, exit_code={hook_input.get('exit_code')}")

        # Check if should trigger
        trigger_start = time.time()
        if not self.should_trigger(hook_input):
            self.logger.info(f"Hook skipped (trigger check: {time.time() - trigger_start:.3f}s)")
            return None
        trigger_time = time.time() - trigger_start
        self.logger.debug(f"Trigger check completed in {trigger_time:.3f}s")

        # Extract output
        output = hook_input.get('output', '')

        if not output:
            self.logger.warning("No output provided in hook input")
            output = "Task completed"

        # Get max length from voice config
        max_length = self.config['voice'].get('max_summary_length', 50)

        # Generate summary with fallback
        DEFAULT_FALLBACK = 'Task completed'

        try:
            summary_start = time.time()
            fallback_msg = self.advanced.get('fallback_message', DEFAULT_FALLBACK)
            summary = self.summarizer.summarize(output, max_length=max_length, fallback=fallback_msg)

            if not summary:
                self.logger.warning("Summarizer returned empty result, using fallback")
                summary = fallback_msg

            summary_time = time.time() - summary_start
            self.logger.info(f"Generated summary in {summary_time:.3f}s: {summary}")

        except Exception as e:
            summary_time = time.time() - summary_start
            self.logger.error(f"Failed to generate summary after {summary_time:.3f}s: {e}")
            summary = self.advanced.get('fallback_message', DEFAULT_FALLBACK)

        # Speak the summary
        try:
            voice_start = time.time()
            is_async = self.config['voice'].get('async', True)

            if is_async:
                self.voice_engine.speak_async(summary)
                voice_time = time.time() - voice_start
                self.logger.info(f"Voice notification triggered (async) in {voice_time:.3f}s")
            else:
                self.voice_engine.speak(summary)
                voice_time = time.time() - voice_start
                self.logger.info(f"Voice notification completed (sync) in {voice_time:.3f}s")

            pipeline_time = time.time() - pipeline_start
            self.logger.info(f"Pipeline completed in {pipeline_time:.3f}s (trigger: {trigger_time:.3f}s, summary: {summary_time:.3f}s, voice: {voice_time:.3f}s)")

            return summary

        except Exception as e:
            voice_time = time.time() - voice_start
            pipeline_time = time.time() - pipeline_start
            self.logger.error(f"Failed to play voice notification after {voice_time:.3f}s (total: {pipeline_time:.3f}s): {e}")
            return None

    def run(self):
        """
        Main entry point - read stdin and process hook.

        Returns:
            Exit code (0 for success, 1 for error)
        """
        try:
            # Read JSON input from stdin
            input_data = sys.stdin.read()

            if not input_data:
                self.logger.error("No input received from stdin")
                return 1

            # Parse JSON
            try:
                hook_input = json.loads(input_data)
            except json.JSONDecodeError as e:
                self.logger.error(f"Failed to parse JSON input: {e}")
                self.logger.debug(f"Raw input: {input_data}")
                return 1

            # Process the hook
            summary = self.process_hook_input(hook_input)

            if summary:
                self.logger.info(f"Hook completed successfully: {summary}")
                return 0
            else:
                self.logger.info("Hook skipped (conditions not met)")
                return 0

        except Exception as e:
            self.logger.error(f"Hook execution failed: {e}", exc_info=True)
            return 1


def main():
    """Main entry point for the hook script."""
    try:
        # Load configuration
        config_path = Path(__file__).parent / 'voice_config.json'

        if not config_path.exists():
            print(f"Error: Configuration file not found: {config_path}", file=sys.stderr)
            return 1

        config = load_config(config_path)

        # Create and run hook
        hook = VoiceNotificationHook(config)
        return hook.run()

    except Exception as e:
        print(f"Fatal error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc(file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
