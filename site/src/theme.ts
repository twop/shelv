import { aurora, frost, polarNight, snowStorm } from "./nord";

function lightenColor(color: string, percent: number) {
  var num = parseInt(color.replace("#", ""), 16),
    amt = Math.round(2.55 * percent),
    R = (num >> 16) + amt,
    B = ((num >> 8) & 0x00ff) + amt,
    G = (num & 0x0000ff) + amt;
  return (
    "#" +
    (
      0x1000000 +
      (R < 255 ? (R < 1 ? 0 : R) : 255) * 0x10000 +
      (B < 255 ? (B < 1 ? 0 : B) : 255) * 0x100 +
      (G < 255 ? (G < 1 ? 0 : G) : 255)
    )
      .toString(16)
      .slice(1)
  );
}

export const colors = {
  // "nord-bg": polarNight.nord0,
  // "nord-bg-dark": lightenColor(polarNight.nord0, -5),
  "nord-bg": lightenColor(polarNight.nord0, -8),
  "nord-bg-dark": lightenColor(polarNight.nord0, -12),
  "nord-text": snowStorm.nord4,
  "nord-h1": frost.nord8,
  "nord-h2": aurora.nord12,
  "nord-input": polarNight.nord1,
  "nord-input-hovered": polarNight.nord3,
  "nord-line-break": polarNight.nord3,
  "nord-input-border": snowStorm.nord4,
  "nord-input-border-hovered": snowStorm.nord6,
  "nord-text-primary": frost.nord8,
  "nord-bg-btn": frost.nord8,
  "nord-bg-btn-hovered": frost.nord10,
  "nord-on-btn": polarNight.nord1,
  "nord-red": aurora.nord12,
};
