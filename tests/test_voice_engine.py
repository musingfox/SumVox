"""
Tests for voice_engine module
"""

import sys
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent / '.claude/hooks'))

import pytest
from voice_engine import VoiceEngine, create_voice_engine


class TestVoiceEngineConfig:
    """Test voice engine configuration and validation."""

    def test_default_config(self):
        """Test engine with default configuration."""
        engine = VoiceEngine()

        assert engine.voice_name == "Ting-Ting"
        assert engine.rate == 200
        assert engine.volume == 75
        assert engine.async_mode is True

    def test_custom_config(self):
        """Test engine with custom configuration."""
        config = {
            'voice_name': 'Mei-Jia',
            'rate': 180,
            'volume': 50,
            'async': False
        }
        engine = VoiceEngine(config)

        assert engine.voice_name == "Mei-Jia"
        assert engine.rate == 180
        assert engine.volume == 50
        assert engine.async_mode is False

    def test_invalid_voice_name(self):
        """Test that invalid voice name falls back to default."""
        config = {'voice_name': 'NonExistentVoice'}
        engine = VoiceEngine(config)

        assert engine.voice_name == VoiceEngine.DEFAULT_VOICE

    def test_invalid_rate(self):
        """Test that invalid rate falls back to default."""
        config = {'rate': 500}  # Out of range
        engine = VoiceEngine(config)

        assert engine.rate == VoiceEngine.DEFAULT_RATE

    def test_invalid_volume(self):
        """Test that invalid volume falls back to default."""
        config = {'volume': 150}  # Out of range
        engine = VoiceEngine(config)

        assert engine.volume == VoiceEngine.DEFAULT_VOLUME


class TestVoiceAvailability:
    """Test voice availability checking."""

    def test_check_default_voice_available(self):
        """Test checking if default voice is available."""
        engine = VoiceEngine()

        # This should work on macOS
        available = engine.is_voice_available()

        # May not work in CI environment, so just check it doesn't crash
        assert isinstance(available, bool)

    def test_get_available_voices(self):
        """Test getting list of available voices."""
        engine = VoiceEngine()

        voices = engine.get_available_voices()

        # Should return a list (may be empty in non-macOS environments)
        assert isinstance(voices, list)


class TestVoiceSpeaking:
    """Test voice speaking functionality."""

    def test_speak_empty_message(self):
        """Test that empty message returns False."""
        engine = VoiceEngine()

        assert engine.speak("") is False
        assert engine.speak("   ") is False

    def test_speak_returns_boolean(self):
        """Test that speak() returns a boolean value."""
        engine = VoiceEngine()

        result = engine.speak("test", blocking=True)

        assert isinstance(result, bool)

    def test_factory_function(self):
        """Test factory function creates engine correctly."""
        config = {'voice_name': 'Sin-ji'}
        engine = create_voice_engine(config)

        assert isinstance(engine, VoiceEngine)
        assert engine.voice_name == 'Sin-ji'


@pytest.mark.skipif(sys.platform != 'darwin', reason="macOS only")
class TestMacOSIntegration:
    """Integration tests for macOS 'say' command."""

    def test_speak_simple_message(self):
        """Test speaking a simple message (macOS only)."""
        engine = VoiceEngine()

        # Synchronous speak
        result = engine.speak("Test message", blocking=True)

        # Should succeed on macOS
        assert result is True

    def test_speak_chinese_message(self):
        """Test speaking Chinese message (macOS only)."""
        engine = VoiceEngine()

        result = engine.speak("測試訊息", blocking=True)

        assert result is True

    def test_async_speak(self):
        """Test asynchronous speaking (macOS only)."""
        engine = VoiceEngine({'async': True})

        result = engine.speak("Async test", blocking=False)

        # Should start successfully
        assert result is True


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
