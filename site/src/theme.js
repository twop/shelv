"use strict";
exports.__esModule = true;
exports.colors = void 0;
var nord_1 = require("./nord");
function lightenColor(color, percent) {
    var num = parseInt(color.replace("#", ""), 16), amt = Math.round(2.55 * percent), R = (num >> 16) + amt, B = ((num >> 8) & 0x00ff) + amt, G = (num & 0x0000ff) + amt;
    return ("#" +
        (0x1000000 +
            (R < 255 ? (R < 1 ? 0 : R) : 255) * 0x10000 +
            (B < 255 ? (B < 1 ? 0 : B) : 255) * 0x100 +
            (G < 255 ? (G < 1 ? 0 : G) : 255))
            .toString(16)
            .slice(1));
}
exports.colors = {
    "nord-bg": nord_1.polarNight.nord0,
    "nord-bg-dark": lightenColor(nord_1.polarNight.nord0, -5),
    "nord-text": nord_1.snowStorm.nord4,
    "nord-h1": nord_1.frost.nord8,
    "nord-h2": nord_1.aurora.nord12,
    "nord-input": nord_1.polarNight.nord1,
    "nord-input-hovered": nord_1.polarNight.nord3,
    "nord-line-break": nord_1.polarNight.nord3,
    "nord-input-border": nord_1.snowStorm.nord4,
    "nord-input-border-hovered": nord_1.snowStorm.nord6,
    "nord-text-primary": nord_1.frost.nord8,
    "nord-bg-btn": nord_1.frost.nord8,
    "nord-bg-btn-hovered": nord_1.frost.nord10,
    "nord-on-btn": nord_1.polarNight.nord1,
    "nord-red": nord_1.aurora.nord12
};
