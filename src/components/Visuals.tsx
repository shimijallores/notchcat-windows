import React from "react";

// 5x7 dot-matrix glyphs matching Mac DotMatrix.swift
const MATRIX_GLYPHS: Record<string, string[]> = {
  "0": ["01110", "10001", "10011", "10101", "11001", "10001", "01110"],
  "1": ["00100", "01100", "00100", "00100", "00100", "00100", "01110"],
  "2": ["01110", "10001", "00001", "00110", "01000", "10000", "11111"],
  "3": ["11110", "00001", "00001", "01110", "00001", "00001", "11110"],
  "4": ["00010", "00110", "01010", "10010", "11111", "00010", "00010"],
  "5": ["11111", "10000", "11110", "00001", "00001", "10001", "01110"],
  "6": ["01110", "10000", "11110", "10001", "10001", "10001", "01110"],
  "7": ["11111", "00001", "00010", "00100", "01000", "01000", "01000"],
  "8": ["01110", "10001", "10001", "01110", "10001", "10001", "01110"],
  "9": ["01110", "10001", "10001", "01111", "00001", "00001", "01110"],
  ":": ["00", "00", "11", "00", "11", "00", "00"],
};

export const DotMatrixDigits: React.FC<{
  text: string;
  dotSize?: number;
  gap?: number;
  color?: string;
  dimColor?: string;
  blinkColon?: boolean;
}> = ({
  text,
  dotSize = 1.6,
  gap = 0.7,
  color = "var(--amber)",
  dimColor = "rgba(255,255,255,0.12)",
  blinkColon = false,
}) => {
  const step = dotSize + gap;
  let offsetX = 0;
  const circles: React.ReactNode[] = [];
  const chars = text.split("");

  chars.forEach((ch, ci) => {
    const glyph = MATRIX_GLYPHS[ch] ?? MATRIX_GLYPHS["0"];
    const cols = glyph[0].length;
    const isColon = ch === ":";
    for (let r = 0; r < 7; r++) {
      for (let c = 0; c < cols; c++) {
        const lit = glyph[r][c] === "1";
        circles.push(
          <circle
            key={`${ci}-${r}-${c}`}
            cx={offsetX + c * step + dotSize / 2}
            cy={r * step + dotSize / 2}
            r={dotSize / 2}
            fill={lit ? color : dimColor}
            className={lit && isColon && blinkColon ? "nc-blink-colon" : undefined}
          />,
        );
      }
    }
    offsetX += cols * step + (ci < chars.length - 1 ? step * 0.7 : 0);
  });

  const width = Math.max(1, offsetX - gap);
  const height = 7 * step - gap;

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className="nc-dot-matrix"
      aria-label={text}
    >
      {circles}
    </svg>
  );
};

// Classic 7-segment polygon mapping (segments A-G)
const SEG_MAP: Record<string, boolean[]> = {
  "0": [true, true, true, true, true, true, false],
  "1": [false, true, true, false, false, false, false],
  "2": [true, true, false, true, true, false, true],
  "3": [true, true, true, true, false, false, true],
  "4": [false, true, true, false, false, true, true],
  "5": [true, false, true, true, false, true, true],
  "6": [true, false, true, true, true, true, true],
  "7": [true, true, true, false, false, false, false],
  "8": [true, true, true, true, true, true, true],
  "9": [true, true, true, true, false, true, true],
};

const SevenSegDigit: React.FC<{
  ch: string;
  litColor: string;
  dimColor: string;
}> = ({ ch, litColor, dimColor }) => {
  const segs = SEG_MAP[ch] ?? SEG_MAP["0"];
  const paths = [
    // A (top horizontal)
    "M 3,1.5 L 15,1.5 L 13,4 L 5,4 Z",
    // B (top-right vertical)
    "M 15.5,2 L 15.5,14 L 13,12.5 L 13,4.5 Z",
    // C (bottom-right vertical)
    "M 15.5,15 L 15.5,27 L 13,24.5 L 13,16.5 Z",
    // D (bottom horizontal)
    "M 3,27.5 L 5,25 L 13,25 L 15,27.5 Z",
    // E (bottom-left vertical)
    "M 2.5,15 L 5,16.5 L 5,24.5 L 2.5,27 Z",
    // F (top-left vertical)
    "M 2.5,2 L 5,4.5 L 5,12.5 L 2.5,14 Z",
    // G (middle horizontal)
    "M 3.2,14.5 L 5,13 L 13,13 L 14.8,14.5 L 13,16 L 5,16 Z",
  ];
  return (
    <svg width="21" height="34" viewBox="0 0 18 29" style={{ display: "block" }}>
      {paths.map((d, idx) => (
        <path key={idx} d={d} fill={segs[idx] ? litColor : dimColor} />
      ))}
    </svg>
  );
};

export const SevenSegmentClock: React.FC<{
  text: string;
  litColor?: string;
  dimColor?: string;
  onClick?: () => void;
  title?: string;
}> = ({
  text,
  litColor = "var(--ink)",
  dimColor = "rgba(27,29,28,0.09)",
  onClick,
  title,
}) => {
  return (
    <div
      className="nc-seven-seg"
      onClick={onClick}
      title={title}
      style={{ cursor: onClick ? "pointer" : "default" }}
    >
      {text.split("").map((ch, idx) =>
        ch === ":" ? (
          <svg
            key={idx}
            width="8"
            height="34"
            viewBox="0 0 8 29"
            style={{ display: "block" }}
          >
            <circle cx="4" cy="10" r="1.9" fill={litColor} />
            <circle cx="4" cy="19.5" r="1.9" fill={litColor} />
          </svg>
        ) : (
          <SevenSegDigit
            key={idx}
            ch={ch}
            litColor={litColor}
            dimColor={dimColor}
          />
        ),
      )}
    </div>
  );
};

export const PaperCard: React.FC<{
  title: string;
  bg: string;
  width: number;
  grain: boolean;
  headerRight?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}> = ({ title, bg, width, grain, headerRight, children, className }) => {
  return (
    <section
      className={`nc-paper-card ${className ?? ""}`}
      style={{ background: bg, width }}
    >
      {grain && <div className="nc-paper-grain" aria-hidden="true" />}
      <header className="nc-card-header">
        <span className="nc-card-title">{title}</span>
        {headerRight && <div className="nc-card-header-right">{headerRight}</div>}
      </header>
      <div className="nc-card-body">{children}</div>
    </section>
  );
};
