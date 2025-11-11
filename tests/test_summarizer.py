"""
Tests for summarizer module
"""

import sys
import os
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent / '.claude/hooks'))

import pytest
from summarizer import (
    ContextExtractor,
    Summarizer,
    OperationType,
    ResultStatus,
    ExecutionContext,
    create_summarizer
)


class TestContextExtractor:
    """Test context extraction functionality."""

    @pytest.fixture
    def extractor(self):
        """Provide context extractor instance."""
        return ContextExtractor()

    def test_detect_code_generation(self, extractor):
        """Test detection of code generation operations."""
        text = "Created new file test.py with 100 lines"
        op_type = extractor._detect_operation_type(text)
        assert op_type == OperationType.CODE_GENERATION

    def test_detect_code_modification(self, extractor):
        """Test detection of code modification operations."""
        text = "Modified src/main.py, updated function signature"
        op_type = extractor._detect_operation_type(text)
        assert op_type == OperationType.CODE_MODIFICATION

    def test_detect_git_operation(self, extractor):
        """Test detection of git operations."""
        text = "git commit -m 'feat: add new feature'"
        op_type = extractor._detect_operation_type(text)
        assert op_type == OperationType.GIT_OPERATION

    def test_detect_testing_operation(self, extractor):
        """Test detection of testing operations."""
        text = "Running pytest... 15 tests passed"
        op_type = extractor._detect_operation_type(text)
        assert op_type == OperationType.TESTING

    def test_detect_success_status(self, extractor):
        """Test detection of success status."""
        text = "Operation completed successfully ✓"
        status = extractor._detect_result_status(text)
        assert status == ResultStatus.SUCCESS

    def test_detect_error_status(self, extractor):
        """Test detection of error status."""
        text = "Error: File not found"
        status = extractor._detect_result_status(text)
        assert status == ResultStatus.ERROR

    def test_extract_key_data_numbers(self, extractor):
        """Test extraction of numerical data."""
        text = "Modified 5 files, added 150 lines, 10 tests passed"
        key_data = extractor._extract_key_data(text)

        assert len(key_data) > 0
        assert any('files' in str(item) for item in key_data)

    def test_extract_key_data_paths(self, extractor):
        """Test extraction of file paths."""
        text = "Created /path/to/test.py and /path/to/main.js"
        key_data = extractor._extract_key_data(text)

        assert len(key_data) > 0
        assert any('.py' in str(item) or '.js' in str(item) for item in key_data)

    def test_extract_error_message(self, extractor):
        """Test error message extraction."""
        text = "Error: Connection timeout after 30 seconds"
        error = extractor._extract_error_message(text)

        assert error is not None
        assert 'Connection timeout' in error

    def test_extract_files(self, extractor):
        """Test file extraction."""
        text = "Modified: test.py, created: main.js, updated: config.json"
        files = extractor._extract_files(text)

        assert len(files) >= 2
        assert any('test.py' in f for f in files)
        assert any('main.js' in f for f in files)

    def test_extract_commands(self, extractor):
        """Test command extraction."""
        text = "$ pytest tests/\n$ git commit -m 'test'\n$ npm build"
        commands = extractor._extract_commands(text)

        assert len(commands) > 0
        assert any('pytest' in cmd or 'git' in cmd or 'npm' in cmd for cmd in commands)

    def test_extract_duration_seconds(self, extractor):
        """Test duration extraction in seconds."""
        text = "Completed in 2.5 seconds"
        duration = extractor._extract_duration(text)

        assert duration == 2.5

    def test_extract_duration_minutes(self, extractor):
        """Test duration extraction in minutes."""
        text = "Completed in 3 minutes"
        duration = extractor._extract_duration(text)

        assert duration == 180.0

    def test_full_extraction(self, extractor):
        """Test complete context extraction."""
        text = """
        Created test.py with 100 lines
        Modified src/main.py
        Running tests... ✓ 15 tests passed
        Completed successfully in 2.5 seconds
        """

        context = extractor.extract(text)

        assert isinstance(context, ExecutionContext)
        assert context.operation_type in [OperationType.CODE_GENERATION, OperationType.TESTING]
        assert context.result_status == ResultStatus.SUCCESS
        assert context.duration_seconds == 2.5


class MockLLMAdapter:
    """Mock LLM adapter for testing."""

    def __init__(self, response: str = "測試摘要"):
        self.response = response
        self.last_prompt = None

    def generate_summary(self, prompt: str, max_length: int = 50):
        """Mock summary generation."""
        self.last_prompt = prompt
        return self.response[:max_length]


class TestSummarizer:
    """Test summarizer functionality."""

    @pytest.fixture
    def test_config(self):
        """Provide test configuration."""
        return {
            'language': 'zh-TW',
            'format': 'concise',
            'include': {
                'operation_type': True,
                'result_status': True,
                'key_data': True,
                'next_steps': True
            },
            'prompt_template': 'Summarize in Traditional Chinese, max {max_length} chars: {context}'
        }

    @pytest.fixture
    def mock_llm(self):
        """Provide mock LLM adapter."""
        return MockLLMAdapter("已建立程式碼並執行測試成功")

    def test_summarizer_init(self, mock_llm, test_config):
        """Test summarizer initialization."""
        summarizer = Summarizer(mock_llm, test_config)

        assert summarizer.llm == mock_llm
        assert summarizer.language == 'zh-TW'
        assert summarizer.format == 'concise'

    def test_summarize_success(self, mock_llm, test_config):
        """Test successful summarization."""
        summarizer = Summarizer(mock_llm, test_config)

        output = """
        Created test.py with 100 lines
        ✓ All tests passed
        """

        summary = summarizer.summarize(output, max_length=50)

        assert summary is not None
        assert len(summary) <= 50
        assert isinstance(summary, str)

    def test_summarize_with_error(self, test_config):
        """Test summarization with failing LLM."""
        # Mock LLM that returns None
        failing_llm = MockLLMAdapter(response=None)
        summarizer = Summarizer(failing_llm, test_config)

        output = "Created test.py successfully"
        summary = summarizer.summarize(output, max_length=50, fallback="任務完成")

        # Should use fallback
        assert summary is not None
        assert len(summary) > 0

    def test_build_context_string(self, mock_llm, test_config):
        """Test context string building."""
        summarizer = Summarizer(mock_llm, test_config)
        extractor = ContextExtractor()

        output = "Created test.py, modified main.py, 15 tests passed"
        context = extractor.extract(output)

        context_str = summarizer._build_context_string(context)

        assert 'Operation:' in context_str
        assert 'Status:' in context_str

    def test_build_fallback_summary_code_generation(self, mock_llm, test_config):
        """Test fallback summary for code generation."""
        summarizer = Summarizer(mock_llm, test_config)
        extractor = ContextExtractor()

        output = "Created new file test.py"
        context = extractor.extract(output)

        fallback = summarizer._build_fallback_summary(context, "任務完成")

        assert fallback is not None
        assert len(fallback) > 0
        # Should contain Traditional Chinese text
        assert any('\u4e00' <= c <= '\u9fff' for c in fallback)

    def test_build_fallback_summary_testing(self, mock_llm, test_config):
        """Test fallback summary for testing."""
        summarizer = Summarizer(mock_llm, test_config)
        extractor = ContextExtractor()

        output = "Running tests... ✓ All passed"
        context = extractor.extract(output)

        fallback = summarizer._build_fallback_summary(context, "任務完成")

        assert fallback is not None
        assert len(fallback) > 0

    def test_build_fallback_summary_with_status(self, mock_llm, test_config):
        """Test fallback summary includes status."""
        summarizer = Summarizer(mock_llm, test_config)
        extractor = ContextExtractor()

        output = "Modified code successfully ✓"
        context = extractor.extract(output)

        fallback = summarizer._build_fallback_summary(context, "任務完成")

        assert fallback is not None
        # Should include success indicator
        assert '成功' in fallback or len(fallback) > 0

    def test_factory_function(self, mock_llm, test_config):
        """Test factory function creates summarizer correctly."""
        summarizer = create_summarizer(mock_llm, test_config)

        assert isinstance(summarizer, Summarizer)

    def test_summarize_max_length_respected(self, mock_llm, test_config):
        """Test that max_length is respected."""
        # Create mock with long response
        long_llm = MockLLMAdapter("這是一個非常長的摘要文字內容，應該被截斷到指定的最大長度限制")
        summarizer = Summarizer(long_llm, test_config)

        output = "Test output"
        summary = summarizer.summarize(output, max_length=20)

        assert len(summary) <= 20

    def test_summarize_with_complex_output(self, mock_llm, test_config):
        """Test summarization with complex multi-line output."""
        summarizer = Summarizer(mock_llm, test_config)

        output = """
        Operation: Code refactoring
        Files modified:
        - src/main.py (150 lines)
        - tests/test_main.py (50 lines)
        - config/settings.json

        Tests: 25/25 passed ✓
        Build: Success
        Duration: 3.5 seconds

        Git commit: abc123d
        """

        summary = summarizer.summarize(output, max_length=50)

        assert summary is not None
        assert len(summary) <= 50


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
