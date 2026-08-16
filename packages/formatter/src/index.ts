/**
 * JavaScript/WASM adapter boundary.
 *
 * The generated WASM binding will be wired here once the Rust formatter core is functional.
 */
export interface FormatOptions {
  braceStyle?: "sameLine" | "nextLine";
  indentWidth?: number;
  useTabs?: boolean;
  lineWidth?: number;
}

export interface FormatDiagnostic {
  message: string;
}

export interface FormatResult {
  text: string;
  diagnostics: FormatDiagnostic[];
}
