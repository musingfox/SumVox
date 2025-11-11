"""
macOS Voice Engine Module

Provides voice notification functionality using macOS 'say' command.
"""

import subprocess
import logging
from typing import Optional, Dict, Any
from pathlib import Path

logger = logging.getLogger(__name__)


class VoiceEngine:
    """
    macOS voice engine using the 'say' command.

    Supports:
    - Multiple Traditional Chinese voices (Ting-Ting, Mei-Jia, Sin-ji)
    - Configurable speech rate and volume
    - Asynchronous voice playback
    - Voice availability checking
    """

    DEFAULT_VOICE = "Ting-Ting"
    DEFAULT_RATE = 200  # words per minute
    DEFAULT_VOLUME = 75  # percentage

    SUPPORTED_VOICES = ["Ting-Ting", "Mei-Jia", "Sin-ji"]

    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """
        Initialize voice engine with configuration.

        Args:
            config: Voice configuration dictionary with keys:
                - voice_name: Voice to use (default: Ting-Ting)
                - rate: Speech rate in wpm (default: 200)
                - volume: Volume 0-100 (default: 75)
                - async: Run asynchronously (default: True)
        """
        self.config = config or {}
        self.voice_name = self.config.get('voice_name', self.DEFAULT_VOICE)
        self.rate = self.config.get('rate', self.DEFAULT_RATE)
        self.volume = self.config.get('volume', self.DEFAULT_VOLUME)
        self.async_mode = self.config.get('async', True)

        # Validate configuration
        self._validate_config()

    def _validate_config(self):
        """Validate voice configuration parameters."""
        if self.voice_name not in self.SUPPORTED_VOICES:
            logger.warning(
                f"Voice '{self.voice_name}' not in supported list. "
                f"Supported: {self.SUPPORTED_VOICES}. Using default: {self.DEFAULT_VOICE}"
            )
            self.voice_name = self.DEFAULT_VOICE

        if not (90 <= self.rate <= 300):
            logger.warning(f"Rate {self.rate} out of range [90-300]. Using default: {self.DEFAULT_RATE}")
            self.rate = self.DEFAULT_RATE

        if not (0 <= self.volume <= 100):
            logger.warning(f"Volume {self.volume} out of range [0-100]. Using default: {self.DEFAULT_VOLUME}")
            self.volume = self.DEFAULT_VOLUME

    def is_voice_available(self, voice_name: Optional[str] = None) -> bool:
        """
        Check if a voice is available on the system.

        Args:
            voice_name: Voice to check (default: configured voice)

        Returns:
            True if voice is available, False otherwise
        """
        voice = voice_name or self.voice_name

        try:
            result = subprocess.run(
                ['say', '-v', '?'],
                capture_output=True,
                text=True,
                timeout=5
            )

            # Check if voice name appears in the output
            available_voices = result.stdout
            return voice in available_voices

        except (subprocess.TimeoutExpired, FileNotFoundError) as e:
            logger.error(f"Failed to check voice availability: {e}")
            return False

    def get_available_voices(self) -> list[str]:
        """
        Get list of all available voices on the system.

        Returns:
            List of voice names
        """
        try:
            result = subprocess.run(
                ['say', '-v', '?'],
                capture_output=True,
                text=True,
                timeout=5
            )

            voices = []
            for line in result.stdout.split('\n'):
                if line.strip():
                    # Extract voice name (first word in each line)
                    voice_name = line.split()[0]
                    voices.append(voice_name)

            return voices

        except (subprocess.TimeoutExpired, FileNotFoundError) as e:
            logger.error(f"Failed to get available voices: {e}")
            return []

    def speak(self, message: str, blocking: Optional[bool] = None) -> bool:
        """
        Speak a message using macOS say command.

        Args:
            message: Text to speak
            blocking: If True, wait for completion. If None, use config.async

        Returns:
            True if command started successfully, False otherwise
        """
        if not message or not message.strip():
            logger.warning("Empty message, skipping voice notification")
            return False

        # Determine if should block
        should_block = not self.async_mode if blocking is None else not blocking

        try:
            # Build say command
            cmd = [
                'say',
                '-v', self.voice_name,
                '-r', str(self.rate),
                message
            ]

            # Note: macOS 'say' doesn't have a direct volume flag in command line
            # Volume is controlled by system settings

            logger.info(
                f"Speaking with voice={self.voice_name}, "
                f"rate={self.rate}, async={not should_block}"
            )

            if should_block:
                # Synchronous execution
                result = subprocess.run(
                    cmd,
                    capture_output=True,
                    text=True,
                    timeout=30
                )

                if result.returncode != 0:
                    logger.error(f"Say command failed: {result.stderr}")
                    return False

                logger.debug("Voice playback completed (blocking)")
                return True

            else:
                # Asynchronous execution
                subprocess.Popen(
                    cmd,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL
                )

                logger.debug("Voice playback started (non-blocking)")
                return True

        except subprocess.TimeoutExpired:
            logger.error("Say command timed out")
            return False

        except FileNotFoundError:
            logger.error("'say' command not found. Is this macOS?")
            return False

        except Exception as e:
            logger.error(f"Unexpected error in speak(): {e}")
            return False

    def test_voice(self) -> bool:
        """
        Test voice playback with a simple message.

        Returns:
            True if test successful, False otherwise
        """
        test_message = "語音測試成功"
        logger.info(f"Testing voice: {self.voice_name}")
        return self.speak(test_message, blocking=True)


def create_voice_engine(config: Optional[Dict[str, Any]] = None) -> VoiceEngine:
    """
    Factory function to create a VoiceEngine instance.

    Args:
        config: Voice configuration dictionary

    Returns:
        Configured VoiceEngine instance
    """
    return VoiceEngine(config)


if __name__ == "__main__":
    # Quick test
    logging.basicConfig(level=logging.INFO)

    engine = VoiceEngine()

    print(f"Testing voice: {engine.voice_name}")
    print(f"Voice available: {engine.is_voice_available()}")
    print(f"Available voices: {engine.get_available_voices()[:5]}...")  # Show first 5

    # Test speak
    success = engine.test_voice()
    print(f"Voice test {'succeeded' if success else 'failed'}")
