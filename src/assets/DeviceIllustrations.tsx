import React, { useId } from "react";

export type DeviceFormFactor =
  | "tws"
  | "open_ear"
  | "over_ear"
  | "speaker"
  | "neckband";

export type SpeakerRgbMode =
  | "solid"
  | "single_breathing"
  | "gradient_breathing"
  | "rhythm";

export interface DeviceColorPreset {
  index: number;
  id: string;
  nameEn: string;
  nameZh: string;
  hex: string;
}

/** Clean-room colorway presets matching common QCY Audio finishes. */
export const DEVICE_COLOR_PRESETS: readonly DeviceColorPreset[] = [
  { index: 0, id: "obsidian_black", nameEn: "Obsidian Black", nameZh: "曜石黑", hex: "#2A2E35" },
  { index: 1, id: "pearl_white", nameEn: "Pearl White", nameZh: "陶瓷白", hex: "#ECEFF4" },
  { index: 2, id: "titanium_gray", nameEn: "Titanium Gray", nameZh: "钛空灰", hex: "#717885" },
  { index: 3, id: "mist_blue", nameEn: "Mist Blue", nameZh: "雾霭蓝", hex: "#5C85B8" },
  { index: 4, id: "warm_sand", nameEn: "Warm Sand", nameZh: "暖沙金", hex: "#D6C5B3" },
  { index: 5, id: "sunset_orange", nameEn: "QCY Orange", nameZh: "活力橙", hex: "#FF7A2F" },
  { index: 6, id: "lavender", nameEn: "Soft Purple", nameZh: "丁香紫", hex: "#9E8CC2" },
];

export interface DevicePalette {
  base: string;
  highlight: string;
  shadow: string;
  deepShadow: string;
  rimStroke: string;
  accentDetail: string;
  isLightBody: boolean;
}

function parseHexRgb(hex: string): [number, number, number] {
  const clean = hex.trim().replace(/^#/, "");
  if (/^[0-9a-fA-F]{6}$/.test(clean)) {
    return [
      parseInt(clean.slice(0, 2), 16),
      parseInt(clean.slice(2, 4), 16),
      parseInt(clean.slice(4, 6), 16),
    ];
  }
  if (/^[0-9a-fA-F]{3}$/.test(clean)) {
    return [
      parseInt(clean[0] + clean[0], 16),
      parseInt(clean[1] + clean[1], 16),
      parseInt(clean[2] + clean[2], 16),
    ];
  }
  return [42, 46, 53];
}

function toHex(r: number, g: number, b: number): string {
  const clampByte = (n: number) => Math.max(0, Math.min(255, Math.round(n)));
  return (
    "#" +
    [clampByte(r), clampByte(g), clampByte(b)]
      .map((v) => v.toString(16).padStart(2, "0"))
      .join("")
  );
}

function mixRgb(
  a: [number, number, number],
  b: [number, number, number],
  t: number
): string {
  return toHex(
    a[0] + (b[0] - a[0]) * t,
    a[1] + (b[1] - a[1]) * t,
    a[2] + (b[2] - a[2]) * t
  );
}

/**
 * Resolves a numeric QCY `colorIndex` to a hex colorway, falling back to Pearl White or Obsidian Black.
 */
export function resolveDeviceColorPreset(colorIndex?: number): DeviceColorPreset {
  if (typeof colorIndex === "number") {
    const match = DEVICE_COLOR_PRESETS.find((p) => p.index === colorIndex);
    if (match) return match;
  }
  return DEVICE_COLOR_PRESETS[0];
}

/**
 * Computes realistic shaded highlight/midtone/shadow/rim tokens for any base device hex color.
 */
export function computeDevicePalette(
  deviceColor: string = "#ECEFF4",
  theme: "light" | "dark" = "dark"
): DevicePalette {
  const rgb = parseHexRgb(deviceColor);
  const luminance = (0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]) / 255;
  const isLightBody = luminance > 0.62;

  const white: [number, number, number] = [255, 255, 255];
  const black: [number, number, number] = [12, 14, 18];

  return {
    base: toHex(rgb[0], rgb[1], rgb[2]),
    highlight: mixRgb(rgb, white, isLightBody ? 0.55 : 0.32),
    shadow: mixRgb(rgb, black, isLightBody ? 0.18 : 0.35),
    deepShadow: mixRgb(rgb, black, isLightBody ? 0.32 : 0.58),
    rimStroke:
      theme === "dark"
        ? isLightBody
          ? "rgba(255,255,255,0.65)"
          : "rgba(255,255,255,0.22)"
        : isLightBody
          ? "rgba(26,26,26,0.18)"
          : "rgba(26,26,26,0.35)",
    accentDetail: isLightBody ? "#64748B" : "#94A3B8",
    isLightBody,
  };
}

export interface DeviceIllustrationProps {
  formFactor: DeviceFormFactor;
  /** Hex tint for the physical device shell (e.g. `#ECEFF4`, `#2A2E35`, `#FF7A2F`). */
  deviceColor?: string;
  /** UI surface theme used to optimize silhouette rim contrast. */
  theme?: "light" | "dark";
  /** Left bud active/connected state (dims when false). */
  leftConnected?: boolean;
  /** Right bud active/connected state (dims when false). */
  rightConnected?: boolean;
  /** Marks left bud as resting inside charging case (applies subtle dim + case state). */
  leftInCase?: boolean;
  /** Marks right bud as resting inside charging case. */
  rightInCase?: boolean;
  leftCharging?: boolean;
  rightCharging?: boolean;
  caseCharging?: boolean;
  /** Shows charging case alongside TWS earbuds (default true). */
  showCase?: boolean;
  /** SP75 Speaker RGB LED ring toggle. */
  rgbEnabled?: boolean;
  /** SP75 Speaker active RGB hex color (default QCY Orange `#FF7A2F`). */
  rgbColor?: string;
  /** SP75 Speaker RGB animation mode. */
  rgbMode?: SpeakerRgbMode;
  /** SVG width/height in pixels (viewBox is 240x160). */
  width?: number;
  height?: number;
  className?: string;
}

function ChargingBolt({
  x,
  y,
  scale = 1,
}: {
  x: number;
  y: number;
  scale?: number;
}) {
  return (
    <g transform={`translate(${x}, ${y}) scale(${scale})`}>
      <circle cx="0" cy="0" r="9.5" fill="#16A34A" stroke="#FFFFFF" strokeWidth="1.4" />
      <path
        d="M 1.2 -5.5 L -3.6 0.8 L 0.2 0.8 L -1.2 5.5 L 3.8 -0.9 L -0.1 -0.9 Z"
        fill="#FFFFFF"
      />
    </g>
  );
}

/**
 * Clean-room theme-aware vector illustration component for all QCY Audio device form factors:
 * `tws`, `open_ear`, `over_ear`, `speaker` (SP75 with live RGB LED ring), and `neckband`.
 */
export const DeviceIllustration: React.FC<DeviceIllustrationProps> = ({
  formFactor,
  deviceColor = "#ECEFF4",
  theme = "dark",
  leftConnected = true,
  rightConnected = true,
  leftInCase = false,
  rightInCase = false,
  leftCharging = false,
  rightCharging = false,
  caseCharging = false,
  showCase = true,
  rgbEnabled = true,
  rgbColor = "#FF7A2F",
  rgbMode = "solid",
  width = 240,
  height = 160,
  className,
}) => {
  const uid = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const pal = computeDevicePalette(deviceColor, theme);

  const leftDimmed = !leftConnected || leftInCase;
  const rightDimmed = !rightConnected || rightInCase;
  const shadowFill = theme === "dark" ? "rgba(0,0,0,0.42)" : "rgba(15,23,42,0.12)";
  const labelColor = theme === "dark" ? "rgba(255,255,255,0.55)" : "rgba(26,26,26,0.5)";

  const shellGradId = `shell-${uid}`;
  const caseGradId = `case-${uid}`;
  const cushionGradId = `cushion-${uid}`;
  const rgbGradId = `rgb-${uid}`;
  const rgbGlowId = `glow-${uid}`;

  return (
    <svg
      width={width}
      height={height}
      viewBox="0 0 240 160"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      role="img"
      aria-label={`QCY ${formFactor.replace("_", " ")} illustration`}
    >
      <defs>
        <linearGradient id={shellGradId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor={pal.highlight} />
          <stop offset="52%" stopColor={pal.base} />
          <stop offset="100%" stopColor={pal.shadow} />
        </linearGradient>

        <linearGradient id={caseGradId} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={pal.highlight} />
          <stop offset="45%" stopColor={pal.base} />
          <stop offset="100%" stopColor={pal.deepShadow} />
        </linearGradient>

        <linearGradient id={cushionGradId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor={pal.isLightBody ? "#D4D8E0" : "#3E434E"} />
          <stop offset="100%" stopColor={pal.isLightBody ? "#9CA3AF" : "#181B20"} />
        </linearGradient>

        <linearGradient id={rgbGradId} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#FF7A2F" />
          <stop offset="35%" stopColor="#FF2E93" />
          <stop offset="70%" stopColor="#3399FF" />
          <stop offset="100%" stopColor="#22C55E" />
        </linearGradient>

        <filter id={rgbGlowId} x="-40%" y="-40%" width="180%" height="180%">
          <feGaussianBlur stdDeviation="4.2" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      {/* Soft ground shadow */}
      <ellipse cx="120" cy="146" rx="82" ry="7" fill={shadowFill} />

      {formFactor === "tws" && (
        <g>
          {/* Left Earbud */}
          <g
            opacity={leftDimmed ? 0.34 : 1}
            style={{ transition: "opacity 220ms ease" }}
          >
            {/* Silicone tip */}
            <ellipse
              cx="45"
              cy="52"
              rx="10"
              ry="13"
              transform="rotate(-14 45 52)"
              fill={pal.deepShadow}
            />
            {/* Acoustic head */}
            <path
              d="M 46 38 C 63 32, 76 44, 74 60 C 72 71, 65 76, 61 81 L 58 118 C 57.5 124, 51 126, 48 122 C 46 119, 46 114, 47 108 L 51 74 C 42 68, 37 56, 46 38 Z"
              fill={`url(#${shellGradId})`}
              stroke={pal.rimStroke}
              strokeWidth="1.2"
            />
            {/* Metallic touch stem accent */}
            <rect
              x="51"
              y="77"
              width="4.5"
              height="26"
              rx="2.2"
              transform="rotate(4 51 77)"
              fill={pal.accentDetail}
              opacity="0.55"
            />
            {/* ANC mic vent */}
            <ellipse cx="64" cy="48" rx="3.2" ry="1.8" fill={pal.deepShadow} />
            <text
              x="54"
              y="138"
              textAnchor="middle"
              fontSize="10"
              fontWeight="600"
              fill={labelColor}
            >
              L
            </text>
            {leftCharging && <ChargingBolt x={68} y={34} scale={0.85} />}
          </g>

          {/* Center Charging Case */}
          {showCase && (
            <g>
              <rect
                x="87"
                y="46"
                width="66"
                height="82"
                rx="22"
                fill={`url(#${caseGradId})`}
                stroke={pal.rimStroke}
                strokeWidth="1.4"
              />
              {/* Lid seam */}
              <path
                d="M 88 74 Q 120 79 152 74"
                stroke={pal.deepShadow}
                strokeWidth="1.4"
                opacity="0.65"
              />
              {/* Thumb notch */}
              <rect
                x="110"
                y="72"
                width="20"
                height="4.5"
                rx="2.2"
                fill={pal.deepShadow}
                opacity="0.5"
              />
              {/* Status LED */}
              <circle
                cx="120"
                cy="91"
                r="2.8"
                fill={caseCharging ? "#22C55E" : "#FF7A2F"}
              />
              {/* Subtle brand mark */}
              <text
                x="120"
                y="112"
                textAnchor="middle"
                fontSize="8.5"
                fontWeight="700"
                letterSpacing="1.6"
                fill={pal.accentDetail}
                opacity="0.7"
              >
                QCY
              </text>
              {caseCharging && <ChargingBolt x={148} y={52} scale={0.85} />}
            </g>
          )}

          {/* Right Earbud */}
          <g
            opacity={rightDimmed ? 0.34 : 1}
            style={{ transition: "opacity 220ms ease" }}
          >
            {/* Silicone tip */}
            <ellipse
              cx="195"
              cy="52"
              rx="10"
              ry="13"
              transform="rotate(14 195 52)"
              fill={pal.deepShadow}
            />
            {/* Acoustic head */}
            <path
              d="M 194 38 C 177 32, 164 44, 166 60 C 168 71, 175 76, 179 81 L 182 118 C 182.5 124, 189 126, 192 122 C 194 119, 194 114, 193 108 L 189 74 C 198 68, 203 56, 194 38 Z"
              fill={`url(#${shellGradId})`}
              stroke={pal.rimStroke}
              strokeWidth="1.2"
            />
            {/* Metallic touch stem accent */}
            <rect
              x="184.5"
              y="77"
              width="4.5"
              height="26"
              rx="2.2"
              transform="rotate(-4 184.5 77)"
              fill={pal.accentDetail}
              opacity="0.55"
            />
            {/* ANC mic vent */}
            <ellipse cx="176" cy="48" rx="3.2" ry="1.8" fill={pal.deepShadow} />
            <text
              x="186"
              y="138"
              textAnchor="middle"
              fontSize="10"
              fontWeight="600"
              fill={labelColor}
            >
              R
            </text>
            {rightCharging && <ChargingBolt x={172} y={34} scale={0.85} />}
          </g>
        </g>
      )}

      {formFactor === "open_ear" && (
        <g>
          {/* Left Open-Ear C-Bridge Clip */}
          <g
            opacity={leftDimmed ? 0.34 : 1}
            style={{ transition: "opacity 220ms ease" }}
          >
            {/* Flexible titanium C-bridge */}
            <path
              d="M 54 68 C 32 70, 32 108, 60 110"
              stroke={`url(#${shellGradId})`}
              strokeWidth="8.5"
              strokeLinecap="round"
            />
            <path
              d="M 54 68 C 32 70, 32 108, 60 110"
              stroke={pal.rimStroke}
              strokeWidth="1.1"
              fill="none"
            />
            {/* Rear ergonomic battery bean */}
            <rect
              x="52"
              y="91"
              width="34"
              height="24"
              rx="12"
              transform="rotate(-12 69 103)"
              fill={`url(#${shellGradId})`}
              stroke={pal.rimStroke}
              strokeWidth="1.2"
            />
            {/* Front acoustic listening sphere */}
            <circle
              cx="66"
              cy="64"
              r="18"
              fill={`url(#${shellGradId})`}
              stroke={pal.rimStroke}
              strokeWidth="1.3"
            />
            <circle cx="72" cy="64" r="6.5" fill={pal.deepShadow} opacity="0.65" />
            <circle cx="62" cy="57" r="3" fill={pal.highlight} opacity="0.7" />
            <text
              x="64"
              y="138"
              textAnchor="middle"
              fontSize="10"
              fontWeight="600"
              fill={labelColor}
            >
              L
            </text>
            {leftCharging && <ChargingBolt x={84} y={48} scale={0.85} />}
          </g>

          {/* Right Open-Ear C-Bridge Clip */}
          <g
            opacity={rightDimmed ? 0.34 : 1}
            style={{ transition: "opacity 220ms ease" }}
          >
            <path
              d="M 186 68 C 208 70, 208 108, 180 110"
              stroke={`url(#${shellGradId})`}
              strokeWidth="8.5"
              strokeLinecap="round"
            />
            <path
              d="M 186 68 C 208 70, 208 108, 180 110"
              stroke={pal.rimStroke}
              strokeWidth="1.1"
              fill="none"
            />
            <rect
              x="154"
              y="91"
              width="34"
              height="24"
              rx="12"
              transform="rotate(12 171 103)"
              fill={`url(#${shellGradId})`}
              stroke={pal.rimStroke}
              strokeWidth="1.2"
            />
            <circle
              cx="174"
              cy="64"
              r="18"
              fill={`url(#${shellGradId})`}
              stroke={pal.rimStroke}
              strokeWidth="1.3"
            />
            <circle cx="168" cy="64" r="6.5" fill={pal.deepShadow} opacity="0.65" />
            <circle cx="178" cy="57" r="3" fill={pal.highlight} opacity="0.7" />
            <text
              x="176"
              y="138"
              textAnchor="middle"
              fontSize="10"
              fontWeight="600"
              fill={labelColor}
            >
              R
            </text>
            {rightCharging && <ChargingBolt x={156} y={48} scale={0.85} />}
          </g>
        </g>
      )}

      {formFactor === "over_ear" && (
        <g opacity={!leftConnected && !rightConnected ? 0.38 : 1}>
          {/* Outer headband arch */}
          <path
            d="M 58 84 C 58 30, 182 30, 182 84"
            stroke={`url(#${shellGradId})`}
            strokeWidth="14"
            strokeLinecap="round"
            fill="none"
          />
          <path
            d="M 58 84 C 58 30, 182 30, 182 84"
            stroke={pal.rimStroke}
            strokeWidth="1.2"
            fill="none"
          />
          {/* Inner padded headband cushion */}
          <path
            d="M 74 47 C 96 34, 144 34, 166 47"
            stroke={`url(#${cushionGradId})`}
            strokeWidth="7"
            strokeLinecap="round"
            fill="none"
          />
          {/* Metallic slider arms */}
          <rect x="52" y="76" width="8" height="22" rx="3" fill={pal.accentDetail} />
          <rect x="180" y="76" width="8" height="22" rx="3" fill={pal.accentDetail} />

          {/* Left Earcup shell & plush cushion */}
          <rect
            x="45"
            y="84"
            width="28"
            height="52"
            rx="14"
            transform="rotate(-8 59 110)"
            fill={`url(#${shellGradId})`}
            stroke={pal.rimStroke}
            strokeWidth="1.3"
          />
          <rect
            x="66"
            y="86"
            width="14"
            height="48"
            rx="7"
            transform="rotate(-8 73 110)"
            fill={`url(#${cushionGradId})`}
          />
          {/* Acoustic ring accent */}
          <ellipse
            cx="57"
            cy="110"
            rx="6"
            ry="13"
            transform="rotate(-8 57 110)"
            stroke={pal.accentDetail}
            strokeWidth="1.2"
            opacity="0.5"
          />

          {/* Right Earcup shell & plush cushion */}
          <rect
            x="167"
            y="84"
            width="28"
            height="52"
            rx="14"
            transform="rotate(8 181 110)"
            fill={`url(#${shellGradId})`}
            stroke={pal.rimStroke}
            strokeWidth="1.3"
          />
          <rect
            x="160"
            y="86"
            width="14"
            height="48"
            rx="7"
            transform="rotate(8 167 110)"
            fill={`url(#${cushionGradId})`}
          />
          <ellipse
            cx="183"
            cy="110"
            rx="6"
            ry="13"
            transform="rotate(8 183 110)"
            stroke={pal.accentDetail}
            strokeWidth="1.2"
            opacity="0.5"
          />
          {/* Right cup LED status */}
          <circle cx="187" cy="126" r="2.2" fill="#0066FF" />

          {(leftCharging || rightCharging || caseCharging) && (
            <ChargingBolt x={195} y={78} scale={0.9} />
          )}
        </g>
      )}

      {formFactor === "speaker" && (
        <g opacity={!leftConnected && !rightConnected ? 0.42 : 1}>
          {/* Cylindrical SP75 Bluetooth Speaker body */}
          <rect
            x="76"
            y="24"
            width="88"
            height="114"
            rx="24"
            fill={`url(#${caseGradId})`}
            stroke={pal.rimStroke}
            strokeWidth="1.5"
          />

          {/* Woven acoustic mesh texture lines */}
          <g opacity="0.22" stroke={pal.isLightBody ? "#1E293B" : "#E2E8F0"} strokeWidth="1">
            {[46, 56, 66, 76, 86, 96, 106, 116].map((y) => (
              <line key={y} x1="84" y1={y} x2="156" y2={y} strokeDasharray="3 4" />
            ))}
          </g>

          {/* Top control deck */}
          <ellipse
            cx="120"
            cy="32"
            rx="34"
            ry="7"
            fill={pal.deepShadow}
            opacity="0.55"
          />
          <circle cx="105" cy="32" r="2.2" fill={pal.highlight} opacity="0.8" />
          <circle cx="120" cy="32" r="2.6" fill="#FF7A2F" />
          <circle cx="135" cy="32" r="2.2" fill={pal.highlight} opacity="0.8" />

          {/* Live RGB LED Ring (top & bottom light rings on SP75) */}
          {rgbEnabled && (
            <g filter={`url(#${rgbGlowId})`}>
              <rect
                x="80"
                y="37"
                width="80"
                height="4.5"
                rx="2.2"
                fill={
                  rgbMode === "gradient_breathing" || rgbMode === "rhythm"
                    ? `url(#${rgbGradId})`
                    : rgbColor
                }
              >
                {rgbMode === "single_breathing" && (
                  <animate
                    attributeName="opacity"
                    values="0.3;1;0.3"
                    dur="2.8s"
                    repeatCount="indefinite"
                  />
                )}
                {rgbMode === "gradient_breathing" && (
                  <animate
                    attributeName="opacity"
                    values="0.45;1;0.45"
                    dur="3.4s"
                    repeatCount="indefinite"
                  />
                )}
                {rgbMode === "rhythm" && (
                  <animate
                    attributeName="opacity"
                    values="0.35;1;0.5;0.95;0.35"
                    dur="0.9s"
                    repeatCount="indefinite"
                  />
                )}
              </rect>

              <rect
                x="80"
                y="124"
                width="80"
                height="5.5"
                rx="2.75"
                fill={
                  rgbMode === "gradient_breathing" || rgbMode === "rhythm"
                    ? `url(#${rgbGradId})`
                    : rgbColor
                }
              >
                {rgbMode === "single_breathing" && (
                  <animate
                    attributeName="opacity"
                    values="0.3;1;0.3"
                    dur="2.8s"
                    repeatCount="indefinite"
                  />
                )}
                {rgbMode === "gradient_breathing" && (
                  <animate
                    attributeName="opacity"
                    values="0.45;1;0.45"
                    dur="3.4s"
                    repeatCount="indefinite"
                  />
                )}
                {rgbMode === "rhythm" && (
                  <animate
                    attributeName="opacity"
                    values="0.35;1;0.5;0.95;0.35"
                    dur="0.9s"
                    repeatCount="indefinite"
                  />
                )}
              </rect>
            </g>
          )}

          {/* Front badge */}
          <rect
            x="102"
            y="72"
            width="36"
            height="18"
            rx="6"
            fill={pal.deepShadow}
            opacity="0.72"
          />
          <text
            x="120"
            y="84"
            textAnchor="middle"
            fontSize="9.5"
            fontWeight="700"
            letterSpacing="1.6"
            fill="#FFFFFF"
          >
            QCY
          </text>

          {(leftCharging || caseCharging) && <ChargingBolt x={160} y={34} scale={0.9} />}
        </g>
      )}

      {formFactor === "neckband" && (
        <g opacity={!leftConnected && !rightConnected ? 0.38 : 1}>
          {/* Flexible U-shaped collar */}
          <path
            d="M 74 114 C 52 72, 68 28, 120 28 C 172 28, 188 72, 166 114"
            stroke={`url(#${shellGradId})`}
            strokeWidth="11"
            strokeLinecap="round"
            fill="none"
          />
          <path
            d="M 74 114 C 52 72, 68 28, 120 28 C 172 28, 188 72, 166 114"
            stroke={pal.rimStroke}
            strokeWidth="1.2"
            fill="none"
          />

          {/* Left & Right control pods */}
          <rect
            x="64"
            y="86"
            width="15"
            height="34"
            rx="7"
            transform="rotate(-14 71 103)"
            fill={`url(#${caseGradId})`}
            stroke={pal.rimStroke}
            strokeWidth="1.2"
          />
          <rect
            x="161"
            y="86"
            width="15"
            height="34"
            rx="7"
            transform="rotate(14 169 103)"
            fill={`url(#${caseGradId})`}
            stroke={pal.rimStroke}
            strokeWidth="1.2"
          />

          {/* Flexible drops to magnetic earbuds */}
          <path
            d="M 76 118 C 82 132, 95 134, 104 124"
            stroke={pal.accentDetail}
            strokeWidth="2.4"
            fill="none"
          />
          <path
            d="M 164 118 C 158 132, 145 134, 136 124"
            stroke={pal.accentDetail}
            strokeWidth="2.4"
            fill="none"
          />

          {/* Magnetic earbud capsules */}
          <circle
            cx="108"
            cy="121"
            r="9"
            fill={`url(#${shellGradId})`}
            stroke={pal.rimStroke}
            strokeWidth="1.2"
          />
          <circle
            cx="132"
            cy="121"
            r="9"
            fill={`url(#${shellGradId})`}
            stroke={pal.rimStroke}
            strokeWidth="1.2"
          />
          <circle cx="104" cy="121" r="3.5" fill={pal.deepShadow} />
          <circle cx="136" cy="121" r="3.5" fill={pal.deepShadow} />

          {(leftCharging || rightCharging || caseCharging) && (
            <ChargingBolt x={174} y={84} scale={0.85} />
          )}
        </g>
      )}
    </svg>
  );
};

export default DeviceIllustration;
