import React from "react";

// ---------------------------------------------------------------------------
// 5x7 dot-matrix text (port of DotMatrix.swift)
// ---------------------------------------------------------------------------

const DOT_ROWS = 7;

function dotGlyph(c: string): string[] {
  switch (c) {
    case "0": return ["01110", "10001", "10011", "10101", "11001", "10001", "01110"];
    case "1": return ["00100", "01100", "00100", "00100", "00100", "00100", "01110"];
    case "2": return ["01110", "10001", "00001", "00010", "00100", "01000", "11111"];
    case "3": return ["11110", "00001", "00001", "01110", "00001", "00001", "11110"];
    case "4": return ["00010", "00110", "01010", "10010", "11111", "00010", "00010"];
    case "5": return ["11111", "10000", "11110", "00001", "00001", "10001", "01110"];
    case "6": return ["00110", "01000", "10000", "11110", "10001", "10001", "01110"];
    case "7": return ["11111", "00001", "00010", "00100", "01000", "01000", "01000"];
    case "8": return ["01110", "10001", "10001", "01110", "10001", "10001", "01110"];
    case "9": return ["01110", "10001", "10001", "01111", "00001", "00010", "01100"];
    case ":": return ["0", "1", "0", "0", "0", "1", "0"];
    default: return Array(7).fill("00000");
  }
}

function dotColumns(text: string): number {
  const widths = text.split("").map((c) => dotGlyph(c)[0].length);
  return widths.reduce((a, b) => a + b, 0) + Math.max(0, widths.length - 1);
}

/** Pitch and padding of the collapsed countdown pill, derived from its height like PillMetrics. */
export const pillPitch = (height: number) => (height * 0.62) / DOT_ROWS;
export const pillHPad = (height: number) => height * 0.62;
export const pillWidth = (text: string, height: number) =>
  dotColumns(text) * pillPitch(height) + pillHPad(height) * 2;

export const DotMatrixText: React.FC<{
  text: string;
  pitch: number;
  lit?: string;
  unlitOpacity?: number;
}> = ({ text, pitch, lit = "#fff", unlitOpacity = 0.1 }) => {
  const cols = dotColumns(text);
  const radius = pitch * 0.36;
  const height = DOT_ROWS * pitch;
  const dots: React.ReactNode[] = [];
  let col = 0;
  text.split("").forEach((ch, ci) => {
    const glyph = dotGlyph(ch);
    const w = glyph[0].length;
    glyph.forEach((row, r) => {
      row.split("").forEach((bit, c) => {
        const on = bit === "1";
        if (!on && w <= 1) return;
        dots.push(
          <circle
            key={`${ci}-${r}-${c}`}
            cx={(col + c + 0.5) * pitch}
            cy={(r + 0.5) * pitch}
            r={radius}
            fill={lit}
            fillOpacity={on ? 1 : unlitOpacity}
          />,
        );
      });
    });
    col += w + 1;
  });
  return (
    <svg
      width={cols * pitch}
      height={height}
      viewBox={`0 0 ${cols * pitch} ${height}`}
      style={{ display: "block", flexShrink: 0 }}
      aria-label={text}
    >
      {dots}
    </svg>
  );
};

// ---------------------------------------------------------------------------
// Seven-segment clock (port of SevenSegment.swift: slanted, tapered segments)
// ---------------------------------------------------------------------------

const SEG_RATIO = 0.52;
const SEG_SPACING = 0.11;
const SEG_COLON = 0.2;
const SEG_SKEW = 0.1;

const SEG_MASK: Record<string, number> = {
  "0": 0b0111111,
  "1": 0b0000110,
  "2": 0b1011011,
  "3": 0b1001111,
  "4": 0b1100110,
  "5": 0b1101101,
  "6": 0b1111101,
  "7": 0b0000111,
  "8": 0b1111111,
  "9": 0b1101111,
};

const hSeg = (x0: number, x1: number, y: number, t: number) =>
  `M${x0},${y} L${x0 + t / 2},${y - t / 2} L${x1 - t / 2},${y - t / 2} L${x1},${y} L${x1 - t / 2},${y + t / 2} L${x0 + t / 2},${y + t / 2} Z`;
const vSeg = (x: number, y0: number, y1: number, t: number) =>
  `M${x},${y0} L${x + t / 2},${y0 + t / 2} L${x + t / 2},${y1 - t / 2} L${x},${y1} L${x - t / 2},${y1 - t / 2} L${x - t / 2},${y0 + t / 2} Z`;

function segmentPath(i: number, w: number, h: number, t: number, x: number): string {
  const gap = t * 0.18;
  const left = x + t / 2;
  const right = x + w - t / 2;
  const top = t / 2;
  const mid = h / 2;
  const bottom = h - t / 2;
  switch (i) {
    case 0: return hSeg(left + gap, right - gap, top, t);
    case 1: return vSeg(right, top + gap, mid - gap, t);
    case 2: return vSeg(right, mid + gap, bottom - gap, t);
    case 3: return hSeg(left + gap, right - gap, bottom, t);
    case 4: return vSeg(left, mid + gap, bottom - gap, t);
    case 5: return vSeg(left, top + gap, mid - gap, t);
    default: return hSeg(left + gap, right - gap, mid, t);
  }
}

export const SevenSegmentClock: React.FC<{
  text: string;
  maxHeight?: number;
  color?: string;
  dim?: boolean;
  onClick?: () => void;
  title?: string;
}> = ({ text, maxHeight = 66, color = "var(--ink)", dim = false, onClick, title }) => {
  const chars = text.split("");
  let unit = SEG_SKEW;
  chars.forEach((ch, i) => {
    unit += ch === ":" ? SEG_COLON : SEG_RATIO;
    if (i < chars.length - 1) unit += SEG_SPACING;
  });

  // Draw at a fixed internal height; CSS scales the SVG to the card.
  const H = 100;
  const t = H * 0.13;
  const shapes: React.ReactNode[] = [];
  let x = 0;
  chars.forEach((ch, ci) => {
    if (ch === ":") {
      const w = H * SEG_COLON;
      const d = t * 1.05;
      [0.32, 0.68].forEach((fy) => {
        shapes.push(
          <rect
            key={`${ci}-${fy}`}
            x={x + (w - d) / 2}
            y={H * fy - d / 2}
            width={d}
            height={d}
            rx={d * 0.2}
            fill={color}
          />,
        );
      });
      x += w + H * SEG_SPACING;
    } else {
      const w = H * SEG_RATIO;
      const mask = SEG_MASK[ch] ?? 0;
      for (let s = 0; s < 7; s++) {
        shapes.push(
          <path
            key={`${ci}-${s}`}
            d={segmentPath(s, w, H, t, x)}
            fill={color}
            fillOpacity={mask & (1 << s) ? 1 : 0.07}
          />,
        );
      }
      x += w + H * SEG_SPACING;
    }
  });

  const width = unit * H;
  return (
    <svg
      className="nc-seven-seg"
      viewBox={`0 0 ${width} ${H}`}
      style={{
        width: "100%",
        maxHeight,
        height: maxHeight,
        opacity: dim ? 0.55 : 1,
        cursor: onClick ? "pointer" : "default",
      }}
      preserveAspectRatio="xMidYMid meet"
      onClick={onClick}
    >
      {title && <title>{title}</title>}
      <g transform={`matrix(1 0 ${-SEG_SKEW} 1 ${SEG_SKEW * H} 0)`}>{shapes}</g>
    </svg>
  );
};

// ---------------------------------------------------------------------------
// Paper card
// ---------------------------------------------------------------------------

export const PaperCard: React.FC<{
  bg: string;
  grain: boolean;
  weight?: number;
  className?: string;
  children: React.ReactNode;
}> = ({ bg, grain, weight = 1, className, children }) => (
  <section
    className={`nc-paper-card ${className ?? ""}`}
    style={{ background: bg, flex: `${weight} 1 0`, transition: "background 300ms ease-out" }}
  >
    {grain && <div className="nc-paper-grain" aria-hidden="true" />}
    <div className="nc-card-body">{children}</div>
  </section>
);
