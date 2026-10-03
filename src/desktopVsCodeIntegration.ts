import type { VsCodeIntegrationHost, VsCodeIntegrationState } from './vsCodeIntegration.js';
import { createVsCodeIntegrationChanges } from './vsCodeIntegrationChanges.js';

const LAUNCH_STATES = new Set(['native', 'wsl', 'cloud-unsupported', 'project-unavailable', 'wsl-extension-missing', 'wsl-extension-unavailable']);
export function parseVsCodeIntegrationState(value: unknown): VsCodeIntegrationState {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid VS Code integration state.');
  const state = value as Record<string, unknown>;
  if (Object.keys(state).sort().join(',') !== 'applicationAvailable,installationScope,installed,launch'
    || typeof state.installed !== 'boolean' || typeof state.applicationAvailable !== 'boolean'
    || state.installationScope !== 'device' || !LAUNCH_STATES.has(String(state.launch))) {
    throw new Error('Invalid VS Code integration state.');
  }
  return state as unknown as VsCodeIntegrationState;
}
export function createDesktopVsCodeIntegration(invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>): VsCodeIntegrationHost {
  // The native registration is installation-wide, independently of project,
  // account or whether the external VS Code application is present.
  const changes = createVsCodeIntegrationChanges('device');
  return {
    async read() { return parseVsCodeIntegrationState(await invoke('read_vscode_integration')); },
    async setInstalled(installed) {
      try { return parseVsCodeIntegrationState(await invoke('set_vscode_integration', { installed })); }
      finally { changes.publish(); }
    },
    subscribeChanges: changes.subscribe,
    async open() {
      if (await invoke('open_external_editor', { id: 'visual_studio_code' }) !== true) throw new Error('Invalid VS Code launch response.');
    },
  };
}
