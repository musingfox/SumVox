"""
Integration Tests for Complete Voice Notification Pipeline

Tests the end-to-end flow:
Claude Code Stop Event → Config Loading → Summarization → Voice Output

These tests verify that all components work together correctly
without requiring actual LLM API calls or voice playback.
"""

import pytest
import json
import sys
from pathlib import Path
from unittest.mock import Mock, patch, MagicMock
from io import StringIO

# Add hooks directory to path
hooks_dir = Path(__file__).parent.parent / '.claude' / 'hooks'
sys.path.insert(0, str(hooks_dir))

from voice_notification import VoiceNotificationHook
from voice_config import load_config


# Test Fixtures: Realistic Claude Code Stop Events
@pytest.fixture
def successful_code_generation_event():
    """Event for successful code file creation."""
    return {
        "output": """Created new file: src/calculator.py (150 lines)
Modified: tests/test_calculator.py
Running pytest...
✓ 15 tests passed
Success! Build completed in 2.5 seconds""",
        "duration": 2.5,
        "exit_code": 0,
        "timestamp": "2025-11-11T14:00:00Z"
    }


@pytest.fixture
def error_detection_event():
    """Event with error in execution."""
    return {
        "output": """Running tests...
Error: Test failed in test_api.py
FAILED tests/test_api.py::test_authentication
AssertionError: Expected 200, got 401
5 passed, 1 failed""",
        "duration": 1.2,
        "exit_code": 1,
        "timestamp": "2025-11-11T14:05:00Z"
    }


@pytest.fixture
def git_operation_event():
    """Event for git operations."""
    return {
        "output": """git add .
git commit -m "feat: Add user authentication"
[main abc123f] feat: Add user authentication
 3 files changed, 45 insertions(+), 10 deletions(-)
git push origin main
Pushed successfully to remote""",
        "duration": 3.0,
        "exit_code": 0,
        "timestamp": "2025-11-11T14:10:00Z"
    }


@pytest.fixture
def quick_task_event():
    """Event for quick task that shouldn't trigger notification."""
    return {
        "output": "File read successfully",
        "duration": 0.5,
        "exit_code": 0,
        "timestamp": "2025-11-11T14:15:00Z"
    }


@pytest.fixture
def malformed_event():
    """Event with missing/malformed data."""
    return {
        "duration": 1.0
        # Missing output and exit_code
    }


@pytest.fixture
def mock_config():
    """Mock configuration for testing."""
    return {
        "version": "1.0.0",
        "enabled": True,
        "llm": {
            "provider": "gemini",
            "models": {
                "primary": "gemini/gemini-2.0-flash-exp",
                "fallback": "claude-3-haiku-20240307"
            },
            "api_keys": {
                "gemini": "test_key_123",
                "anthropic": "test_key_456"
            },
            "parameters": {
                "max_tokens": 100,
                "temperature": 0.3,
                "timeout": 10
            },
            "cost_control": {
                "daily_limit_usd": 0.10,
                "usage_tracking": False
            }
        },
        "voice": {
            "engine": "macos_say",
            "voice_name": "Ting-Ting",
            "rate": 200,
            "volume": 75,
            "max_summary_length": 50,
            "async": True
        },
        "triggers": {
            "on_completion": True,
            "on_error": True,
            "min_duration_seconds": 1.0,
            "error_keywords": ["Error:", "Failed:", "Exception:", "FAILED"]
        },
        "summarization": {
            "language": "zh-TW",
            "format": "concise",
            "include": {
                "operation_type": True,
                "result_status": True,
                "key_data": True,
                "next_steps": True
            },
            "prompt_template": "Summarize in Traditional Chinese, max {max_length} chars: {context}"
        },
        "logging": {
            "enabled": False  # Disable logging during tests
        },
        "advanced": {
            "cache_summaries": False,
            "retry_attempts": 3,
            "fallback_message": "任務完成"
        }
    }


class TestIntegrationPipeline:
    """Test complete end-to-end pipeline."""

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_successful_code_generation_flow(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        successful_code_generation_event
    ):
        """Test complete flow for successful code generation."""
        # Setup mocks
        mock_llm = Mock()
        mock_llm.generate_summary.return_value = "已建立計算機程式碼，15 個測試通過"
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Process event
        summary = hook.process_hook_input(successful_code_generation_event)

        # Verify summary was generated
        assert summary is not None
        assert len(summary) > 0

        # Verify LLM was called
        mock_llm.generate_summary.assert_called_once()
        call_args = mock_llm.generate_summary.call_args
        assert "code_generation" in call_args[0][0].lower() or "created" in call_args[0][0].lower()

        # Verify voice engine was called
        mock_voice_engine.speak_async.assert_called_once_with(summary)

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_error_detection_flow(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        error_detection_event
    ):
        """Test complete flow for error detection."""
        # Setup mocks
        mock_llm = Mock()
        mock_llm.generate_summary.return_value = "測試失敗，認證錯誤"
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Process event
        summary = hook.process_hook_input(error_detection_event)

        # Verify notification was triggered despite error
        assert summary is not None

        # Verify error was detected
        assert hook._contains_error(error_detection_event['output'])

        # Verify voice engine was called
        mock_voice_engine.speak_async.assert_called_once()

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_git_operation_flow(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        git_operation_event
    ):
        """Test complete flow for git operations."""
        # Setup mocks
        mock_llm = Mock()
        mock_llm.generate_summary.return_value = "已提交並推送程式碼到遠端"
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Process event
        summary = hook.process_hook_input(git_operation_event)

        # Verify summary generated
        assert summary is not None

        # Verify LLM received git-related context
        call_args = mock_llm.generate_summary.call_args
        assert "git" in call_args[0][0].lower() or "commit" in call_args[0][0].lower()

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_quick_task_skipped(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        quick_task_event
    ):
        """Test that quick tasks below minimum duration are skipped."""
        # Setup mocks
        mock_llm = Mock()
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Process event
        summary = hook.process_hook_input(quick_task_event)

        # Verify notification was skipped
        assert summary is None

        # Verify LLM was NOT called
        mock_llm.generate_summary.assert_not_called()

        # Verify voice engine was NOT called
        mock_voice_engine.speak_async.assert_not_called()

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_llm_failure_graceful_fallback(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        successful_code_generation_event
    ):
        """Test graceful fallback when LLM fails."""
        # Setup mocks - LLM raises exception
        mock_llm = Mock()
        mock_llm.generate_summary.side_effect = Exception("LLM API timeout")
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Process event
        summary = hook.process_hook_input(successful_code_generation_event)

        # Verify fallback summary was used
        assert summary is not None
        assert "任務完成" in summary or "已建立程式碼" in summary

        # Verify voice engine was still called with fallback
        mock_voice_engine.speak_async.assert_called_once()

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_malformed_input_handling(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        malformed_event
    ):
        """Test handling of malformed input data."""
        # Setup mocks
        mock_llm = Mock()
        mock_llm.generate_summary.return_value = "任務完成"
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Process event - should handle gracefully
        summary = hook.process_hook_input(malformed_event)

        # Verify it handled missing data
        assert summary is not None

        # Verify voice engine was called
        mock_voice_engine.speak_async.assert_called_once()

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_disabled_notifications(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        successful_code_generation_event
    ):
        """Test that disabled notifications are skipped."""
        # Disable notifications
        mock_config['enabled'] = False

        # Setup mocks
        mock_llm = Mock()
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Process event
        summary = hook.process_hook_input(successful_code_generation_event)

        # Verify notification was skipped
        assert summary is None

        # Verify nothing was called
        mock_llm.generate_summary.assert_not_called()
        mock_voice_engine.speak_async.assert_not_called()

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_voice_engine_failure_handling(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        successful_code_generation_event
    ):
        """Test handling when voice engine fails."""
        # Setup mocks - voice engine raises exception
        mock_llm = Mock()
        mock_llm.generate_summary.return_value = "測試摘要"
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine.speak_async.side_effect = Exception("Voice engine error")
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Process event - should not crash
        summary = hook.process_hook_input(successful_code_generation_event)

        # Verify summary was generated but voice failed
        # Should return None since voice playback failed
        assert summary is None

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_sync_voice_mode(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        successful_code_generation_event
    ):
        """Test synchronous voice playback mode."""
        # Configure sync mode
        mock_config['voice']['async'] = False

        # Setup mocks
        mock_llm = Mock()
        mock_llm.generate_summary.return_value = "測試摘要"
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Process event
        summary = hook.process_hook_input(successful_code_generation_event)

        # Verify sync method was called
        mock_voice_engine.speak.assert_called_once_with(summary)
        mock_voice_engine.speak_async.assert_not_called()


class TestStdinIntegration:
    """Test stdin input processing."""

    @patch('voice_notification.VoiceNotificationHook.process_hook_input')
    @patch('voice_notification.load_config')
    @patch('sys.stdin', new_callable=StringIO)
    def test_valid_json_stdin(
        self,
        mock_stdin,
        mock_load_config,
        mock_process,
        mock_config,
        successful_code_generation_event
    ):
        """Test reading valid JSON from stdin."""
        # Setup stdin with JSON
        mock_stdin.write(json.dumps(successful_code_generation_event))
        mock_stdin.seek(0)

        # Setup config
        mock_load_config.return_value = mock_config
        mock_process.return_value = "測試摘要"

        # Import and create hook
        from voice_notification import VoiceNotificationHook

        with patch('voice_notification.create_llm_adapter'), \
             patch('voice_notification.VoiceEngine'):
            hook = VoiceNotificationHook(mock_config)
            exit_code = hook.run()

        # Verify success
        assert exit_code == 0
        mock_process.assert_called_once()

    @patch('voice_notification.load_config')
    @patch('sys.stdin', new_callable=StringIO)
    def test_invalid_json_stdin(
        self,
        mock_stdin,
        mock_load_config,
        mock_config
    ):
        """Test handling of invalid JSON from stdin."""
        # Setup stdin with invalid JSON
        mock_stdin.write("{invalid json")
        mock_stdin.seek(0)

        # Setup config
        mock_load_config.return_value = mock_config

        # Import and create hook
        from voice_notification import VoiceNotificationHook

        with patch('voice_notification.create_llm_adapter'), \
             patch('voice_notification.VoiceEngine'):
            hook = VoiceNotificationHook(mock_config)
            exit_code = hook.run()

        # Verify error handling
        assert exit_code == 1

    @patch('voice_notification.load_config')
    @patch('sys.stdin', new_callable=StringIO)
    def test_empty_stdin(
        self,
        mock_stdin,
        mock_load_config,
        mock_config
    ):
        """Test handling of empty stdin."""
        # Empty stdin
        mock_stdin.write("")
        mock_stdin.seek(0)

        # Setup config
        mock_load_config.return_value = mock_config

        # Import and create hook
        from voice_notification import VoiceNotificationHook

        with patch('voice_notification.create_llm_adapter'), \
             patch('voice_notification.VoiceEngine'):
            hook = VoiceNotificationHook(mock_config)
            exit_code = hook.run()

        # Verify error handling
        assert exit_code == 1


class TestPerformance:
    """Test performance characteristics of the pipeline."""

    @patch('voice_notification.VoiceEngine')
    @patch('voice_notification.create_llm_adapter')
    def test_pipeline_performance(
        self,
        mock_llm_factory,
        mock_voice_engine_class,
        mock_config,
        successful_code_generation_event
    ):
        """Test that pipeline completes in reasonable time."""
        import time

        # Setup mocks with realistic delays
        mock_llm = Mock()
        def slow_generate(*args, **kwargs):
            time.sleep(0.1)  # Simulate LLM call
            return "測試摘要"
        mock_llm.generate_summary.side_effect = slow_generate
        mock_llm_factory.return_value = mock_llm

        mock_voice_engine = Mock()
        mock_voice_engine_class.return_value = mock_voice_engine

        # Create hook
        hook = VoiceNotificationHook(mock_config)

        # Measure time
        start_time = time.time()
        summary = hook.process_hook_input(successful_code_generation_event)
        elapsed_time = time.time() - start_time

        # Verify completed successfully
        assert summary is not None

        # Verify reasonable performance (should be < 1 second for mocked calls)
        assert elapsed_time < 1.0, f"Pipeline took too long: {elapsed_time}s"


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
