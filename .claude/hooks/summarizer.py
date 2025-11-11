"""
Claude Code Output Summarization Engine

Extracts key information from Claude Code execution and generates
concise voice-friendly summaries in Traditional Chinese.
"""

import logging
import json
import re
from typing import Dict, Any, Optional, List
from dataclasses import dataclass
from enum import Enum

from llm_adapter import LLMAdapter

logger = logging.getLogger(__name__)


class OperationType(Enum):
    """Types of Claude Code operations."""
    CODE_GENERATION = "code_generation"
    CODE_MODIFICATION = "code_modification"
    FILE_OPERATION = "file_operation"
    GIT_OPERATION = "git_operation"
    SEARCH = "search"
    ANALYSIS = "analysis"
    TESTING = "testing"
    BUILD = "build"
    DEBUGGING = "debugging"
    DOCUMENTATION = "documentation"
    UNKNOWN = "unknown"


class ResultStatus(Enum):
    """Execution result status."""
    SUCCESS = "success"
    PARTIAL_SUCCESS = "partial_success"
    FAILURE = "failure"
    ERROR = "error"
    CANCELLED = "cancelled"
    UNKNOWN = "unknown"


@dataclass
class ExecutionContext:
    """Structured execution context from Claude Code."""
    raw_output: str
    operation_type: OperationType
    result_status: ResultStatus
    key_data: List[str]
    error_message: Optional[str]
    files_modified: List[str]
    commands_executed: List[str]
    duration_seconds: Optional[float]


class ContextExtractor:
    """Extracts structured information from Claude Code output."""

    def __init__(self):
        # Patterns for operation detection
        self.operation_patterns = {
            OperationType.CODE_GENERATION: [
                r'creat(ed|ing).*\.(py|js|ts|java|go|rs)',
                r'generat(ed|ing) code',
                r'writ(ing|e) new file',
                r'created new file'
            ],
            OperationType.CODE_MODIFICATION: [
                r'modif(y|ied|ying)',
                r'updat(ed|ing)',
                r'edit(ed|ing)',
                r'refactor(ed|ing)'
            ],
            OperationType.GIT_OPERATION: [
                r'git (commit|push|pull|clone|branch|merge|checkout)',
                r'committ(ed|ing)',
                r'push(ed|ing) to'
            ],
            OperationType.FILE_OPERATION: [
                r'(read|writ(e|ing)|delet(e|ing)|mov(e|ing))\s+file',
                r'file.*created',
                r'director(y|ies) created'
            ],
            OperationType.SEARCH: [
                r'search(ing|ed) for',
                r'grep',
                r'find.*file'
            ],
            OperationType.TESTING: [
                r'test(s|ing|ed)',
                r'pytest',
                r'jest',
                r'unit test',
                r'integration test'
            ],
            OperationType.BUILD: [
                r'build(ing|s)?',
                r'compil(e|ing|ed)',
                r'npm (run|build)',
                r'bun build'
            ],
            OperationType.DEBUGGING: [
                r'debug(ging)?',
                r'fix(ed|ing) (bug|error)',
                r'troubleshoot'
            ]
        }

        # Patterns for status detection
        self.success_patterns = [
            r'success(ful|fully)?',
            r'complete(d)?',
            r'done',
            r'passed',
            r'✓',
            r'✅'
        ]

        self.error_patterns = [
            r'error:',
            r'failed:',
            r'exception:',
            r'fatal:',
            r'❌',
            r'✗'
        ]

    def extract(self, raw_output: str) -> ExecutionContext:
        """
        Extract structured information from raw output.

        Args:
            raw_output: Raw text output from Claude Code

        Returns:
            ExecutionContext with extracted information
        """
        operation_type = self._detect_operation_type(raw_output)
        result_status = self._detect_result_status(raw_output)
        key_data = self._extract_key_data(raw_output)
        error_message = self._extract_error_message(raw_output)
        files_modified = self._extract_files(raw_output)
        commands_executed = self._extract_commands(raw_output)
        duration = self._extract_duration(raw_output)

        return ExecutionContext(
            raw_output=raw_output,
            operation_type=operation_type,
            result_status=result_status,
            key_data=key_data,
            error_message=error_message,
            files_modified=files_modified,
            commands_executed=commands_executed,
            duration_seconds=duration
        )

    def _detect_operation_type(self, text: str) -> OperationType:
        """Detect the type of operation from text."""
        text_lower = text.lower()

        for op_type, patterns in self.operation_patterns.items():
            for pattern in patterns:
                if re.search(pattern, text_lower):
                    return op_type

        return OperationType.UNKNOWN

    def _detect_result_status(self, text: str) -> ResultStatus:
        """Detect the execution result status."""
        text_lower = text.lower()

        # Check for errors first
        for pattern in self.error_patterns:
            if re.search(pattern, text_lower):
                return ResultStatus.ERROR

        # Check for success
        for pattern in self.success_patterns:
            if re.search(pattern, text_lower):
                return ResultStatus.SUCCESS

        # Check for partial success
        if 'partial' in text_lower or 'warning' in text_lower:
            return ResultStatus.PARTIAL_SUCCESS

        return ResultStatus.UNKNOWN

    def _extract_key_data(self, text: str) -> List[str]:
        """Extract key data points from text."""
        key_data = []

        # Extract numbers (could be counts, sizes, etc.)
        numbers = re.findall(r'\b\d+\s+(files?|lines?|tests?|errors?|warnings?)\b', text)
        key_data.extend(numbers)

        # Extract important paths (first few) - more flexible pattern
        paths = re.findall(r'[/~]?[\w\-./]+\.(?:py|js|ts|json|md|txt|yaml|yml)', text)
        key_data.extend(paths[:3])  # Limit to 3 paths

        # Extract git commit hashes
        commits = re.findall(r'\b[0-9a-f]{7,40}\b', text)
        key_data.extend(commits[:2])  # Limit to 2 commits

        return key_data

    def _extract_error_message(self, text: str) -> Optional[str]:
        """Extract error message if present."""
        # Try to find error message
        error_match = re.search(r'(Error|Exception|Failed):\s*(.+?)(?:\n|$)', text, re.IGNORECASE)
        if error_match:
            return error_match.group(2).strip()

        return None

    def _extract_files(self, text: str) -> List[str]:
        """Extract modified/created files."""
        files = re.findall(r'(?:modified|created|updated|edited):\s*(\S+\.\w+)', text, re.IGNORECASE)
        return list(set(files))[:5]  # Dedupe and limit to 5

    def _extract_commands(self, text: str) -> List[str]:
        """Extract executed commands."""
        # Look for shell commands (starting with $ or in code blocks)
        commands = re.findall(r'(?:[$>]\s*)([a-z][\w\-]+(?:\s+[^\n]+)?)', text)
        return list(set(commands))[:3]  # Dedupe and limit to 3

    def _extract_duration(self, text: str) -> Optional[float]:
        """Extract execution duration if present."""
        # Look for duration patterns
        duration_match = re.search(r'(\d+(?:\.\d+)?)\s*(s|sec|second)s?', text, re.IGNORECASE)
        if duration_match:
            return float(duration_match.group(1))

        # Look for minutes
        duration_match = re.search(r'(\d+(?:\.\d+)?)\s*(m|min|minute)s?', text, re.IGNORECASE)
        if duration_match:
            return float(duration_match.group(1)) * 60

        return None


class Summarizer:
    """
    Generate concise voice-friendly summaries of Claude Code execution.
    """

    def __init__(self, llm_adapter: LLMAdapter, config: Dict[str, Any]):
        """
        Initialize summarizer.

        Args:
            llm_adapter: LLM adapter for text generation
            config: Summarization configuration from voice_config.json
        """
        self.llm = llm_adapter
        self.config = config
        self.extractor = ContextExtractor()

        self.language = config.get('language', 'zh-TW')
        self.format = config.get('format', 'concise')
        self.include = config.get('include', {})
        self.prompt_template = config.get(
            'prompt_template',
            'Summarize in Traditional Chinese, max {max_length} chars: {context}'
        )

    def summarize(
        self,
        output: str,
        max_length: int = 50,
        fallback: str = "任務完成"
    ) -> str:
        """
        Generate summary from Claude Code output.

        Args:
            output: Raw output text from Claude Code
            max_length: Maximum summary length in characters
            fallback: Fallback message if summarization fails

        Returns:
            Concise summary in Traditional Chinese
        """
        # Extract structured context
        context = self.extractor.extract(output)

        # Build context string for LLM
        context_str = self._build_context_string(context)

        # Generate summary with LLM
        prompt = self._build_prompt(context_str, max_length)

        try:
            summary = self.llm.generate_summary(prompt, max_length=max_length)

            if summary:
                return summary
            else:
                logger.warning("LLM returned empty summary, using fallback")
                return self._build_fallback_summary(context, fallback)

        except Exception as e:
            logger.error(f"Failed to generate summary: {e}")
            return self._build_fallback_summary(context, fallback)

    def _build_context_string(self, context: ExecutionContext) -> str:
        """Build context string from extracted information."""
        parts = []

        # Add operation type
        if self.include.get('operation_type', True):
            parts.append(f"Operation: {context.operation_type.value}")

        # Add result status
        if self.include.get('result_status', True):
            parts.append(f"Status: {context.result_status.value}")

        # Add key data
        if self.include.get('key_data', True) and context.key_data:
            parts.append(f"Data: {', '.join(context.key_data)}")

        # Add error if present
        if context.error_message:
            parts.append(f"Error: {context.error_message}")

        # Add files
        if context.files_modified:
            parts.append(f"Files: {', '.join(context.files_modified)}")

        # Add duration
        if context.duration_seconds:
            parts.append(f"Duration: {context.duration_seconds}s")

        return '; '.join(parts)

    def _build_prompt(self, context: str, max_length: int) -> str:
        """Build LLM prompt from context."""
        return self.prompt_template.format(
            max_length=max_length,
            context=context
        )

    def _build_fallback_summary(self, context: ExecutionContext, default: str) -> str:
        """Build simple fallback summary without LLM."""
        # Map operation types to Traditional Chinese
        op_map = {
            OperationType.CODE_GENERATION: "已建立程式碼",
            OperationType.CODE_MODIFICATION: "已修改程式碼",
            OperationType.FILE_OPERATION: "已處理檔案",
            OperationType.GIT_OPERATION: "已執行 Git 操作",
            OperationType.TESTING: "已執行測試",
            OperationType.BUILD: "已建置專案",
            OperationType.SEARCH: "已搜尋檔案",
            OperationType.DEBUGGING: "已修正錯誤",
            OperationType.UNKNOWN: default
        }

        # Map status to Traditional Chinese
        status_map = {
            ResultStatus.SUCCESS: "成功",
            ResultStatus.PARTIAL_SUCCESS: "部分成功",
            ResultStatus.FAILURE: "失敗",
            ResultStatus.ERROR: "錯誤",
            ResultStatus.UNKNOWN: ""
        }

        operation_text = op_map.get(context.operation_type, default)
        status_text = status_map.get(context.result_status, "")

        if status_text:
            return f"{operation_text}{status_text}"
        else:
            return operation_text


def create_summarizer(llm_adapter: LLMAdapter, config: Dict[str, Any]) -> Summarizer:
    """
    Factory function to create Summarizer instance.

    Args:
        llm_adapter: Configured LLMAdapter instance
        config: Summarization configuration

    Returns:
        Configured Summarizer instance
    """
    return Summarizer(llm_adapter, config)


if __name__ == "__main__":
    # Quick test
    logging.basicConfig(level=logging.INFO)

    from llm_adapter import create_llm_adapter

    # Load configuration
    config_path = '/Users/nickhuang/workspace/claude-voice/.claude/hooks/voice_config.json'
    with open(config_path, 'r') as f:
        full_config = json.load(f)

    # Create components
    llm_adapter = create_llm_adapter(full_config['llm'])
    summarizer = create_summarizer(llm_adapter, full_config['summarization'])

    # Test with sample output
    test_output = """
    Created test_example.py with 150 lines
    Modified src/main.py
    Running pytest...
    ✓ 15 tests passed
    Success! Build completed in 2.5s
    """

    summary = summarizer.summarize(test_output, max_length=50)
    print(f"Summary: {summary}")
