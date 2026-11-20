import React from "react";
import { SoundscapeEngine } from "../audio/soundscapeEngine";
import { SoundscapePanel } from "../components";
import { useI18n } from "../i18n";
import { useQcyStore } from "../store/useQcyStore";

/**
 * Persistent shared soundscape synthesizer instance so ambient Focus Mode audio
 * continues playing seamlessly while navigating between tabs in the desktop app.
 */
export const sharedSoundscapeEngine = new SoundscapeEngine();

export interface SoundscapesPageProps {
  engine?: SoundscapeEngine;
}

export const SoundscapesPage: React.FC<SoundscapesPageProps> = ({
  engine = sharedSoundscapeEngine,
}) => {
  const { snapshot, setFocusWhiteNoise, startLocalOta } = useQcyStore();
  const { t } = useI18n();

  const supportsOnboard = Boolean(
    snapshot?.active_model.capabilities.focus_mode_white_noise
  );
  const isDisconnected = snapshot?.connection_state === "disconnected";
  const hw = snapshot?.focus_white_noise;

  return (
    <div className="space-y-5">
      <SoundscapePanel
        engine={engine}
        supportsOnboardWhiteNoise={supportsOnboard}
        onboardState={
          hw
            ? {
                playing: hw.hardware_playing,
                trackIndex: hw.hardware_track_index,
                volume: hw.hardware_volume,
                syncing: hw.sync_progress !== null,
                syncProgressPercent: hw.sync_progress ?? 0,
              }
            : undefined
        }
        disabled={isDisconnected}
        onOnboardTogglePlay={(playing) =>
          void setFocusWhiteNoise(playing, undefined, undefined)
        }
        onOnboardTrackChange={(trackIndex) =>
          void setFocusWhiteNoise(undefined, trackIndex, undefined)
        }
        onOnboardVolumeChange={(volume) =>
          void setFocusWhiteNoise(undefined, undefined, volume)
        }
        onSyncTonePack={(file) => {
          void (async () => {
            const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
            await startLocalOta(file.name, bytes);
          })();
        }}
        t={t}
      />
    </div>
  );
};
