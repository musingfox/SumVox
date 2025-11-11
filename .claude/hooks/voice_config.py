"""
Configuration loader for voice notification hook.

Handles loading and validation of voice_config.json.
"""

import json
import logging
from pathlib import Path
from typing import Dict, Any

logger = logging.getLogger(__name__)


def load_config(config_path: Path) -> Dict[str, Any]:
    """
    Load configuration from JSON file.

    Args:
        config_path: Path to voice_config.json

    Returns:
        Configuration dictionary

    Raises:
        FileNotFoundError: If config file doesn't exist
        json.JSONDecodeError: If config file is invalid JSON
        ValueError: If config validation fails
    """
    if not config_path.exists():
        raise FileNotFoundError(f"Configuration file not found: {config_path}")

    with open(config_path, 'r', encoding='utf-8') as f:
        config = json.load(f)

    # Basic validation
    validate_config(config)

    return config


def validate_config(config: Dict[str, Any]) -> None:
    """
    Validate configuration structure.

    Args:
        config: Configuration dictionary to validate

    Raises:
        ValueError: If validation fails
    """
    # Check required top-level keys
    required_keys = ['llm', 'voice', 'triggers', 'summarization']

    for key in required_keys:
        if key not in config:
            raise ValueError(f"Missing required configuration key: {key}")

    # Validate LLM config
    llm_config = config['llm']
    if 'models' not in llm_config:
        raise ValueError("LLM config missing 'models'")

    # Validate voice config
    voice_config = config['voice']
    required_voice_keys = ['engine', 'voice_name']
    for key in required_voice_keys:
        if key not in voice_config:
            raise ValueError(f"Voice config missing required key: {key}")

    logger.info("Configuration validation passed")


if __name__ == "__main__":
    # Test loading
    config_path = Path(__file__).parent / 'voice_config.json'
    config = load_config(config_path)
    print(f"Config loaded successfully: version {config.get('version')}")
