/** Bundled DDS integration registration, separate from the VS Code application. */
export const VSCODE_INTEGRATION_ID = 'altifigence.vscode';
export const VSCODE_INTEGRATION_VERSION = '1.0.0';

export interface VsCodeIntegrationState {
  installed: boolean;
  installationScope: 'device' | 'project';
  applicationAvailable: boolean | null;
  launch: 'native' | 'wsl' | 'cloud-unsupported' | 'project-unavailable'
    | 'wsl-extension-missing' | 'wsl-extension-unavailable';
}

export interface VsCodeIntegrationHost {
  read(): Promise<VsCodeIntegrationState>;
  setInstalled(installed: boolean): Promise<VsCodeIntegrationState>;
  /** Invalidation only: each host rereads its own installation authority. */
  subscribeChanges?(onChange: () => void): () => void;
  /** Uses only the host's current immutable workspace, never a renderer path. */
  open(): Promise<void>;
}

export function hasVsCodeIntegration(document: unknown): boolean {
  if (!document || typeof document !== 'object' || Array.isArray(document)) return false;
  const extensions = (document as Record<string, unknown>).extensions;
  if (!extensions || typeof extensions !== 'object' || Array.isArray(extensions)) return false;
  const entry = (extensions as Record<string, unknown>)[VSCODE_INTEGRATION_ID];
  return Boolean(entry && typeof entry === 'object' && !Array.isArray(entry)
    && (entry as Record<string, unknown>).version === VSCODE_INTEGRATION_VERSION
    && (entry as Record<string, unknown>).installed === true);
}

export function vsCodeIntegrationPatch(installed: boolean): Record<string, unknown> {
  return { extensions: { [VSCODE_INTEGRATION_ID]: installed
    ? { version: VSCODE_INTEGRATION_VERSION, installed: true } : null } };
}
