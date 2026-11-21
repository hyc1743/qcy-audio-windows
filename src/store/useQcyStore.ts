import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import {
  qcyClient,
  type QcyIpcClient,
  type SetAncPayload,
  type SetAudioEnhancersPayload,
  type SetModesPayload,
  type SetMultipointPayload,
  type SetRgbLightingPayload,
  type SimulatorPatch,
} from "../ipc/client";
import type {
  AiAncMatrix,
  AppSettings,
  AppSnapshot,
  CommandOutcome,
  DeviceModelProfile,
  DiscoveredDevice,
  EqPresetItem,
  FitTestResult,
  MutualExclusionPrompt,
  OtaState,
  PopupTriggerPayload,
  ProtocolLogEntry,
  WireParametricBand,
} from "../types/qcy";

export interface PendingConflictAction {
  prompt: MutualExclusionPrompt;
  confirm: () => Promise<void>;
  cancel: () => void;
}

export interface QcyStoreValue {
  client: QcyIpcClient;
  snapshot: AppSnapshot | null;
  models: DeviceModelProfile[];
  scannedDevices: DiscoveredDevice[];
  protocolLogs: ProtocolLogEntry[];
  popupPayload: PopupTriggerPayload | null;
  pendingConflict: PendingConflictAction | null;
  isScanning: boolean;
  loading: boolean;
  error: string | null;

  refresh: () => Promise<void>;
  clearError: () => void;
  dismissPopup: () => void;

  startScan: () => Promise<void>;
  stopScan: () => Promise<void>;
  refreshPairedDevices: () => Promise<void>;
  connectDevice: (address: string, modelId?: string) => Promise<void>;
  disconnectDevice: () => Promise<void>;
  removeSavedDevice: (address: string) => Promise<void>;
  openWindowsBtSettings: () => Promise<void>;

  setAncState: (payload: SetAncPayload) => Promise<void>;
  setAiAnc: (enabled: boolean, matrix?: AiAncMatrix) => Promise<void>;
  setAncButtonCycle: (modes: string[]) => Promise<void>;

  applyEqPreset: (presetId: string) => Promise<void>;
  setGraphicEq: (gainsDb: number[], masterGainDb?: number) => Promise<void>;
  setParametricEq: (bands: WireParametricBand[], masterGainDb?: number) => Promise<void>;
  saveCustomEq: (preset: EqPresetItem) => Promise<void>;
  deleteCustomEq: (presetId: string) => Promise<void>;
  importEqJson: (json: string) => Promise<void>;
  exportEqJson: (presetId?: string) => Promise<string>;
  applyPersonalizedEq: (
    leftDb: number[],
    rightDb: number[],
    combinedDb: number[],
    balance?: number
  ) => Promise<void>;
  setAudioEnhancers: (payload: SetAudioEnhancersPayload) => Promise<void>;

  setButtonAction: (keyId: number, eventId: number, cmd: number) => Promise<void>;
  resetButtons: () => Promise<void>;
  setTouchConfig: (touchEnable?: boolean, touchSensitivity?: number) => Promise<void>;
  setHeadControls: (patch: {
    enabled?: boolean;
    nod_cmd?: number;
    shake_cmd?: number;
    tilt_left_cmd?: number;
    tilt_right_cmd?: number;
  }) => Promise<void>;

  setModes: (payload: SetModesPayload) => Promise<void>;
  setMultipoint: (payload: SetMultipointPayload) => Promise<void>;
  setRgbLighting: (payload: SetRgbLightingPayload) => Promise<void>;
  setFocusWhiteNoise: (playing?: boolean, trackIndex?: number, volume?: number) => Promise<void>;
  runFitTest: () => Promise<FitTestResult | null>;
  setDeviceColor: (colorIndex: number) => Promise<void>;
  renameDevice: (name: string) => Promise<void>;
  resetDevice: (kind: "soft_reset" | "clear_pairing" | "factory_reset") => Promise<void>;
  startLocalOta: (
    filePathOrName: string,
    bytes?: number[] | Uint8Array
  ) => Promise<OtaState | null>;
  cancelOta: () => Promise<void>;

  setSimulatorEnabled: (enabled: boolean) => Promise<void>;
  setSimulatorModel: (modelId: string) => Promise<void>;
  updateSimulatorState: (patch: SimulatorPatch) => Promise<void>;
  triggerSimulatorPopup: () => Promise<void>;

  updateAppSettings: (patch: Partial<AppSettings>) => Promise<void>;
  clearProtocolLogs: () => Promise<void>;
}

const QcyStoreContext = createContext<QcyStoreValue | null>(null);

export interface QcyStoreProviderProps {
  client?: QcyIpcClient;
  children: React.ReactNode;
}

export const QcyStoreProvider: React.FC<QcyStoreProviderProps> = ({
  client = qcyClient,
  children,
}) => {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [models, setModels] = useState<DeviceModelProfile[]>([]);
  const [scannedDevices, setScannedDevices] = useState<DiscoveredDevice[]>([]);
  const [protocolLogs, setProtocolLogs] = useState<ProtocolLogEntry[]>([]);
  const [popupPayload, setPopupPayload] = useState<PopupTriggerPayload | null>(null);
  const [pendingConflict, setPendingConflict] = useState<PendingConflictAction | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const handleOutcome = useCallback(
    (outcome: CommandOutcome, retryForced?: () => Promise<CommandOutcome>) => {
      setSnapshot(outcome.snapshot);
      if (!outcome.applied && outcome.conflict_prompt && retryForced) {
        const prompt = outcome.conflict_prompt;
        setPendingConflict({
          prompt,
          confirm: async () => {
            setPendingConflict(null);
            try {
              const forced = await retryForced();
              setSnapshot(forced.snapshot);
            } catch (e) {
              setError(e instanceof Error ? e.message : String(e));
            }
          },
          cancel: () => setPendingConflict(null),
        });
      }
    },
    []
  );

  const runSafe = useCallback(async <T,>(fn: () => Promise<T>): Promise<T | null> => {
    try {
      setError(null);
      return await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return null;
    }
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [snap, catalog, logs] = await Promise.all([
        client.getAppSnapshot(),
        client.listModels(),
        client.getProtocolLogs(),
      ]);
      setSnapshot(snap);
      setModels(catalog);
      setProtocolLogs(logs);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    void refresh();
    const unlisteners: Array<() => void> = [];
    let cancelled = false;

    const bind = async () => {
      const u1 = await client.onStateUpdated((snap) => {
        if (!cancelled) setSnapshot(snap);
      });
      const u2 = await client.onPopupTriggered((payload) => {
        if (!cancelled) setPopupPayload(payload);
      });
      const u3 = await client.onScanDiscovered((dev) => {
        if (!cancelled) {
          setScannedDevices((prev) => {
            const idx = prev.findIndex((d) => d.address === dev.address);
            if (idx >= 0) {
              const next = [...prev];
              next[idx] = dev;
              return next;
            }
            return [...prev, dev];
          });
        }
      });
      const u4 = await client.onProtocolLog((entry) => {
        if (!cancelled) {
          setProtocolLogs((prev) => [...prev.slice(-199), entry]);
        }
      });
      if (cancelled) {
        u1();
        u2();
        u3();
        u4();
      } else {
        unlisteners.push(u1, u2, u3, u4);
      }
    };

    void bind();
    return () => {
      cancelled = true;
      for (const fn of unlisteners) fn();
    };
  }, [client, refresh]);

  const value = useMemo<QcyStoreValue>(
    () => ({
      client,
      snapshot,
      models,
      scannedDevices,
      protocolLogs,
      popupPayload,
      pendingConflict,
      isScanning,
      loading,
      error,

      refresh,
      clearError: () => setError(null),
      dismissPopup: () => {
        setPopupPayload(null);
        void client.hidePopup();
      },

      startScan: async () => {
        setIsScanning(true);
        const paired = await runSafe(() => client.listPairedDevices());
        if (paired) setScannedDevices(paired);
        await runSafe(() => client.startScan());
      },
      stopScan: async () => {
        setIsScanning(false);
        await runSafe(() => client.stopScan());
      },
      refreshPairedDevices: async () => {
        const paired = await runSafe(() => client.listPairedDevices());
        if (!paired) return;
        setScannedDevices((prev) => {
          const byAddr = new Map<string, DiscoveredDevice>();
          for (const d of prev) byAddr.set(d.address.toUpperCase(), d);
          for (const d of paired) {
            const key = d.address.toUpperCase();
            const existing = byAddr.get(key);
            byAddr.set(key, existing ? { ...existing, ...d, rssi: d.rssi ?? existing.rssi } : d);
          }
          return Array.from(byAddr.values());
        });
      },
      connectDevice: async (address, modelId) => {
        const snap = await runSafe(() => client.connectDevice(address, modelId));
        if (snap) setSnapshot(snap);
      },
      disconnectDevice: async () => {
        const snap = await runSafe(() => client.disconnectDevice());
        if (snap) setSnapshot(snap);
      },
      removeSavedDevice: async (address) => {
        const snap = await runSafe(() => client.removeSavedDevice(address));
        if (snap) setSnapshot(snap);
      },
      openWindowsBtSettings: async () => {
        await runSafe(() => client.openWindowsBtSettings());
      },

      setAncState: async (payload) => {
        const res = await runSafe(() => client.setAncState(payload));
        if (res) handleOutcome(res, () => client.setAncState({ ...payload, force: true }));
      },
      setAiAnc: async (enabled, matrix) => {
        const res = await runSafe(() => client.setAiAnc(enabled, matrix));
        if (res) handleOutcome(res);
      },
      setAncButtonCycle: async (modes) => {
        const res = await runSafe(() => client.setAncButtonCycle(modes));
        if (res) handleOutcome(res);
      },

      applyEqPreset: async (presetId) => {
        const res = await runSafe(() => client.applyEqPreset(presetId));
        if (res) handleOutcome(res);
      },
      setGraphicEq: async (gainsDb, masterGainDb) => {
        const res = await runSafe(() => client.setGraphicEq(gainsDb, masterGainDb));
        if (res) handleOutcome(res);
      },
      setParametricEq: async (bands, masterGainDb) => {
        const res = await runSafe(() => client.setParametricEq(bands, masterGainDb));
        if (res) handleOutcome(res);
      },
      saveCustomEq: async (preset) => {
        const snap = await runSafe(() => client.saveCustomEq(preset));
        if (snap) setSnapshot(snap);
      },
      deleteCustomEq: async (presetId) => {
        const snap = await runSafe(() => client.deleteCustomEq(presetId));
        if (snap) setSnapshot(snap);
      },
      importEqJson: async (json) => {
        const snap = await runSafe(() => client.importEqJson(json));
        if (snap) setSnapshot(snap);
      },
      exportEqJson: async (presetId) => {
        return (await runSafe(() => client.exportEqJson(presetId))) ?? "";
      },
      applyPersonalizedEq: async (leftDb, rightDb, combinedDb, balance) => {
        const res = await runSafe(() =>
          client.applyPersonalizedEq(leftDb, rightDb, combinedDb, balance)
        );
        if (res) handleOutcome(res);
      },
      setAudioEnhancers: async (payload) => {
        const res = await runSafe(() => client.setAudioEnhancers(payload));
        if (res) handleOutcome(res, () => client.setAudioEnhancers({ ...payload, force: true }));
      },

      setButtonAction: async (keyId, eventId, cmd) => {
        const res = await runSafe(() => client.setButtonAction(keyId, eventId, cmd));
        if (res) handleOutcome(res);
      },
      resetButtons: async () => {
        const res = await runSafe(() => client.resetButtons());
        if (res) handleOutcome(res);
      },
      setTouchConfig: async (touchEnable, touchSensitivity) => {
        const res = await runSafe(() => client.setTouchConfig(touchEnable, touchSensitivity));
        if (res) handleOutcome(res);
      },
      setHeadControls: async (patch) => {
        const res = await runSafe(() => client.setHeadControls(patch));
        if (res) handleOutcome(res);
      },

      setModes: async (payload) => {
        const res = await runSafe(() => client.setModes(payload));
        if (res) handleOutcome(res, () => client.setModes({ ...payload, force: true }));
      },
      setMultipoint: async (payload) => {
        const res = await runSafe(() => client.setMultipoint(payload));
        if (res) handleOutcome(res, () => client.setMultipoint({ ...payload, force: true }));
      },
      setRgbLighting: async (payload) => {
        const res = await runSafe(() => client.setRgbLighting(payload));
        if (res) handleOutcome(res);
      },
      setFocusWhiteNoise: async (playing, trackIndex, volume) => {
        const res = await runSafe(() => client.setFocusWhiteNoise(playing, trackIndex, volume));
        if (res) handleOutcome(res);
      },
      runFitTest: async () => runSafe(() => client.runFitTest()),
      setDeviceColor: async (colorIndex) => {
        const snap = await runSafe(() => client.setDeviceColor(colorIndex));
        if (snap) setSnapshot(snap);
      },
      renameDevice: async (name) => {
        const res = await runSafe(() => client.renameDevice(name));
        if (res) handleOutcome(res);
      },
      resetDevice: async (kind) => {
        const res = await runSafe(() => client.resetDevice(kind));
        if (res) handleOutcome(res);
      },
      startLocalOta: async (filePathOrName, bytes) =>
        runSafe(() => client.startLocalOta(filePathOrName, bytes)),
      cancelOta: async () => {
        await runSafe(() => client.cancelOta());
      },

      setSimulatorEnabled: async (enabled) => {
        const snap = await runSafe(() => client.setSimulatorEnabled(enabled));
        if (snap) {
          setSnapshot(snap);
          if (!enabled) {
            setScannedDevices((prev) => prev.filter((d) => !d.is_virtual));
          }
        }
      },
      setSimulatorModel: async (modelId) => {
        const snap = await runSafe(() => client.setSimulatorModel(modelId));
        if (snap) setSnapshot(snap);
      },
      updateSimulatorState: async (patch) => {
        const snap = await runSafe(() => client.updateSimulatorState(patch));
        if (snap) setSnapshot(snap);
      },
      triggerSimulatorPopup: async () => {
        const payload = await runSafe(() => client.triggerSimulatorPopup());
        if (payload) setPopupPayload(payload);
      },

      updateAppSettings: async (patch) => {
        const snap = await runSafe(() => client.updateAppSettings(patch));
        if (snap) setSnapshot(snap);
      },
      clearProtocolLogs: async () => {
        await runSafe(() => client.clearProtocolLogs());
        setProtocolLogs([]);
      },
    }),
    [
      client,
      snapshot,
      models,
      scannedDevices,
      protocolLogs,
      popupPayload,
      pendingConflict,
      isScanning,
      loading,
      error,
      refresh,
      runSafe,
      handleOutcome,
    ]
  );

  return React.createElement(QcyStoreContext.Provider, { value }, children);
};

export function useQcyStore(): QcyStoreValue {
  const ctx = useContext(QcyStoreContext);
  if (!ctx) {
    throw new Error("useQcyStore must be used inside <QcyStoreProvider>");
  }
  return ctx;
}
