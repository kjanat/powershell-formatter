/**
 * Browser and Node.js bindings for the standalone PowerShell formatter.
 *
 * The runtime is initialized once and cached; repeated calls reuse the same
 * WebAssembly instance. All entry points are async so browser and Node share
 * one API shape.
 */

/** Where opening braces of statement blocks go. */
export type BraceStyle = 'sameLine' | 'nextLine';

/** Placement of `else`/`elseif`/`catch`/`finally` after `}`. */
export type BranchKeywordPlacement = 'nextLine' | 'cuddled';

/** Pipeline continuation indentation (PSScriptAnalyzer semantics). */
export type PipelineIndentation =
	| 'increaseIndentationForFirstPipeline'
	| 'increaseIndentationAfterEveryPipeline'
	| 'noIndentation'
	| 'none';

/** Output newline handling. */
export type EndOfLine = 'auto' | 'lf' | 'crlf';

/**
 * Formatting configuration. Every field is optional; defaults reproduce
 * PSScriptAnalyzer's `CodeFormatting` preset. Field names mirror the Rust
 * `FormatOptions` model (validated mechanically in the package tests).
 */
export interface FormatOptions {
	indentWidth?: number;
	useTabs?: boolean;
	lineWidth?: number;
	placeOpenBrace?: boolean;
	placeCloseBrace?: boolean;
	braceStyle?: BraceStyle;
	newlineAfterOpenBrace?: boolean;
	ignoreOneLineBlock?: boolean;
	branchKeywordPlacement?: BranchKeywordPlacement;
	noEmptyLineBeforeCloseBrace?: boolean;
	spaceBeforeOpenBrace?: boolean;
	spaceInsideBrace?: boolean;
	spaceAfterKeyword?: boolean;
	spaceAroundOperator?: boolean;
	spaceAfterSeparator?: boolean;
	spaceAroundPipe?: boolean;
	collapseSpaceAroundPipe?: boolean;
	collapseSpaceBetweenParameters?: boolean;
	ignoreAssignmentInHashtable?: boolean;
	indentation?: boolean;
	pipelineIndentation?: PipelineIndentation;
	alignAssignment?: boolean;
	keywordCasing?: boolean;
	operatorCasing?: boolean;
	commandCasing?: boolean;
	endOfLine?: EndOfLine;
	finalNewline?: boolean | null;
}

/** A formatting diagnostic. */
export interface Diagnostic {
	message: string;
	/** Stable machine-readable code, e.g. `"unterminated-string"`. */
	code: string;
	severity: 'info' | 'warning' | 'error';
	/** 1-based line of the diagnostic. */
	line: number;
	/** 1-based UTF-16 column of the diagnostic. */
	column: number;
	/** UTF-8 byte offsets into the source. */
	start: number;
	end: number;
}

export interface FormatResult {
	/** The formatted source (identical to the input when `formatted` is false). */
	text: string;
	/** False when the input was preserved unchanged for safety. */
	formatted: boolean;
	diagnostics: Diagnostic[];
}

/** A 1-based line/column range, like `Invoke-Formatter -Range`. */
export interface FormatRange {
	startLine: number;
	startColumn: number;
	endLine: number;
	endColumn: number;
}

/** Canonical command/parameter casing data. */
export interface CommandCatalog {
	commands: Record<string, string[]>;
}

/**
 * Explicitly initialize the WebAssembly runtime. Optional — the formatting
 * functions initialize on first use. In the browser, `input` may be a URL or
 * `WebAssembly.Module`/`Response` to load the wasm from a custom location.
 */
export function initialize(input?: unknown): Promise<void>;

/** Format a complete PowerShell source string. */
export function format(
	source: string,
	options?: FormatOptions,
	catalog?: CommandCatalog,
): Promise<FormatResult>;

/** Format only the given range, leaving the rest byte-identical. */
export function formatRange(
	source: string,
	range: FormatRange,
	options?: FormatOptions,
	catalog?: CommandCatalog,
): Promise<FormatResult>;

/** The formatter's version. */
export function version(): Promise<string>;

/** The complete default configuration. */
export function defaultOptions(): Promise<Required<FormatOptions>>;
