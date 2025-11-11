"""
Tests for voice_notification hook module
"""

import sys
import os
import json
import tempfile
from pathlib import Path
from io import StringIO

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent / '.claude/hooks'))

import pytest
from voice_notification import VoiceNotificationHook, setup_logging
from voice_config import load_config, validate_config


class MockLLMAdapter:
    """Mock LLM adapter for testing."""

    def __init__(self, response: str = "測試摘要"):
        self.response = response

    def generate_summary(self, prompt: str, max_length: int = 50):
        return self.response[:max_length]


class MockVoiceEngine:
    """Mock voice engine for testing."""

    def __init__(self, config):
        self.config = config
        self.last_text = None
        self.speak_count = 0
        self.async_count = 0

    def speak(self, text: str):
        """Record synchronous speak call."""
        self.last_text = text
        self.speak_count += 1

    def speak_async(self, text: str):
        """Record asynchronous speak call."""
        self.last_text = text
        self.async_count += 1


class TestVoiceConfig:
    """Test configuration loading and validation."""

    def test_load_valid_config(self):
        """Test loading valid configuration file."""
        config_path = Path(__file__).parent.parent / '.claude/hooks/voice_config.json'

        if not config_path.exists():
            pytest.skip("Config file not found")

        config = load_config(config_path)

        assert 'version' in config
        assert 'llm' in config
        assert 'voice' in config
        assert 'triggers' in config
        assert 'summarization' in config

    def test_load_nonexistent_config(self):
        """Test loading non-existent config file."""
        with pytest.raises(FileNotFoundError):
            load_config(Path('/nonexistent/config.json'))

    def test_validate_valid_config(self):
        """Test validation of valid config."""
        valid_config = {
            'llm': {
                'models': {'primary': 'test-model'}
            },
            'voice': {
                'engine': 'macos_say',
                'voice_name': 'Ting-Ting'
            },
            'triggers': {},
            'summarization': {}
        }

        # Should not raise
        validate_config(valid_config)

    def test_validate_missing_llm(self):
        """Test validation fails on missing LLM config."""
        invalid_config = {
            'voice': {'engine': 'test', 'voice_name': 'test'},
            'triggers': {},
            'summarization': {}
        }

        with pytest.raises(ValueError, match="Missing required configuration key: llm"):
            validate_config(invalid_config)

    def test_validate_missing_voice_engine(self):
        """Test validation fails on missing voice engine."""
        invalid_config = {
            'llm': {'models': {'primary': 'test'}},
            'voice': {'voice_name': 'test'},  # Missing 'engine'
            'triggers': {},
            'summarization': {}
        }

        with pytest.raises(ValueError, match="Voice config missing required key: engine"):
            validate_config(invalid_config)


class TestVoiceNotificationHook:
    """Test main hook orchestrator."""

    @pytest.fixture
    def test_config(self):
        """Provide test configuration."""
        return {
            'enabled': True,
            'llm': {
                'models': {
                    'primary': 'test-model'
                },
                'api_keys': {},
                'parameters': {
                    'max_tokens': 100,
                    'temperature': 0.3,
                    'timeout': 10
                },
                'cost_control': {
                    'daily_limit_usd': 0.10,
                    'usage_tracking': False
                }
            },
            'voice': {
                'engine': 'macos_say',
                'voice_name': 'Ting-Ting',
                'rate': 200,
                'volume': 75,
                'max_summary_length': 50,
                'async': True
            },
            'triggers': {
                'on_completion': True,
                'on_error': True,
                'min_duration_seconds': 0,
                'error_keywords': ['Error:', 'Failed:', 'Exception:']
            },
            'summarization': {
                'language': 'zh-TW',
                'format': 'concise',
                'include': {},
                'prompt_template': 'Test: {context}'
            },
            'logging': {
                'enabled': False
            },
            'advanced': {
                'fallback_message': '任務完成'
            }
        }

    @pytest.fixture
    def mock_hook(self, test_config, monkeypatch):
        """Create hook with mocked components."""
        # Monkey patch component creation
        from summarizer import Summarizer
        from llm_adapter import LLMAdapter

        mock_llm = MockLLMAdapter("測試摘要")
        mock_voice = MockVoiceEngine(test_config['voice'])

        def mock_create_llm(config):
            return mock_llm

        def mock_create_summarizer(llm, config):
            return Summarizer(llm, config)

        def mock_voice_engine(config):
            return mock_voice

        monkeypatch.setattr('voice_notification.create_llm_adapter', mock_create_llm)
        monkeypatch.setattr('voice_notification.VoiceEngine', lambda config: mock_voice)

        hook = VoiceNotificationHook(test_config)
        hook.voice_engine = mock_voice  # Ensure we can access the mock

        return hook, mock_voice

    def test_hook_init(self, test_config):
        """Test hook initialization."""
        # This test requires actual components, so we'll mock them
        from unittest.mock import patch

        with patch('voice_notification.create_llm_adapter'):
            with patch('voice_notification.create_summarizer'):
                with patch('voice_notification.VoiceEngine'):
                    hook = VoiceNotificationHook(test_config)

                    assert hook.config == test_config
                    assert hook.triggers == test_config['triggers']
                    assert hook.advanced == test_config['advanced']

    def test_should_trigger_on_completion(self, mock_hook):
        """Test trigger on completion."""
        hook, _ = mock_hook

        hook_input = {
            'output': 'Task completed successfully',
            'duration': 5.0,
            'exit_code': 0
        }

        assert hook.should_trigger(hook_input) is True

    def test_should_trigger_on_error(self, mock_hook):
        """Test trigger on error."""
        hook, _ = mock_hook

        hook_input = {
            'output': 'Error: Something went wrong',
            'duration': 5.0,
            'exit_code': 1
        }

        assert hook.should_trigger(hook_input) is True

    def test_should_not_trigger_below_min_duration(self, mock_hook):
        """Test no trigger when below minimum duration."""
        hook, _ = mock_hook
        hook.triggers['min_duration_seconds'] = 10.0

        hook_input = {
            'output': 'Quick task',
            'duration': 5.0,
            'exit_code': 0
        }

        assert hook.should_trigger(hook_input) is False

    def test_should_not_trigger_when_disabled(self, mock_hook):
        """Test no trigger when globally disabled."""
        hook, _ = mock_hook
        hook.config['enabled'] = False

        hook_input = {
            'output': 'Task completed',
            'duration': 5.0,
            'exit_code': 0
        }

        assert hook.should_trigger(hook_input) is False

    def test_contains_error_keyword(self, mock_hook):
        """Test error keyword detection."""
        hook, _ = mock_hook

        assert hook._contains_error('Error: File not found') is True
        assert hook._contains_error('Failed: Connection timeout') is True
        assert hook._contains_error('Success! All done') is False

    def test_process_hook_input_success(self, mock_hook):
        """Test processing successful execution."""
        hook, mock_voice = mock_hook

        hook_input = {
            'output': 'Created test.py successfully',
            'duration': 5.0,
            'exit_code': 0
        }

        summary = hook.process_hook_input(hook_input)

        assert summary is not None
        assert mock_voice.async_count == 1
        assert mock_voice.last_text is not None

    def test_process_hook_input_skipped(self, mock_hook):
        """Test skipping notification when conditions not met."""
        hook, mock_voice = mock_hook
        hook.triggers['min_duration_seconds'] = 100.0

        hook_input = {
            'output': 'Quick task',
            'duration': 1.0,
            'exit_code': 0
        }

        summary = hook.process_hook_input(hook_input)

        assert summary is None
        assert mock_voice.async_count == 0

    def test_process_hook_input_empty_output(self, mock_hook):
        """Test handling empty output."""
        hook, mock_voice = mock_hook

        hook_input = {
            'output': '',
            'duration': 5.0,
            'exit_code': 0
        }

        summary = hook.process_hook_input(hook_input)

        # Should use fallback
        assert summary is not None
        assert mock_voice.async_count == 1

    def test_process_hook_input_sync_mode(self, mock_hook):
        """Test synchronous voice playback."""
        hook, mock_voice = mock_hook
        hook.config['voice']['async'] = False

        hook_input = {
            'output': 'Task completed',
            'duration': 5.0,
            'exit_code': 0
        }

        summary = hook.process_hook_input(hook_input)

        assert summary is not None
        assert mock_voice.speak_count == 1
        assert mock_voice.async_count == 0


class TestHookIntegration:
    """Integration tests for the complete hook."""

    def test_hook_with_json_input(self, tmp_path):
        """Test hook with JSON input via stdin."""
        # This is a simplified integration test
        # Full test would require mocking stdin and the entire execution flow

        test_input = {
            'output': 'Test execution completed',
            'duration': 5.0,
            'exit_code': 0,
            'timestamp': '2025-11-11T14:00:00Z'
        }

        # Verify JSON serialization works
        json_str = json.dumps(test_input)
        parsed = json.loads(json_str)

        assert parsed['output'] == test_input['output']
        assert parsed['duration'] == test_input['duration']
        assert parsed['exit_code'] == test_input['exit_code']


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
