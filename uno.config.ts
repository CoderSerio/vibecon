import { defineConfig, presetUno } from "unocss";

export default defineConfig({
  presets: [presetUno()],
  shortcuts: {
    "app-shell":
      "mx-auto max-w-[1180px] p-[44px_32px] max-[720px]:p-[25px_15px]",
    "app-header": "flex items-start justify-between gap-5",
    "app-title": "m-0 text-[40px] tracking-[-1.6px]",
    "section-title": "m-0 text-base",
    eyebrow: "mb-[5px] text-[11px] font-800 tracking-[1.5px] text-[#7de6c4]",
    subtitle: "mt-[6px] text-[#9faeac]",
    "app-button":
      "cursor-pointer rounded-[9px] border border-[#407067] bg-[#1b332e] px-[14px] py-[10px] font-inherit text-[#e9fffa] hover:border-[#7de6c4] hover:bg-[#254d44]",
    secondary: "app-button bg-transparent text-[#b7c9c5]",
    panel:
      "mt-7 rounded-xl border border-[#2e4541] bg-[rgba(25,34,33,.88)] p-[18px] shadow-[0_14px_30px_rgba(0,0,0,.18)]",
    "section-heading": "flex items-start justify-between gap-5",
    status: "text-right text-[13px] text-[#9faeac]",
    connected: "text-[#7de6c4]",
    error: "text-[#ff9c96]",
    controllers: "mt-4 flex flex-wrap gap-[10px] text-sm text-[#9faeac]",
    controller: "app-button grid min-w-[220px] gap-1 text-left",
    "selected-controller": "border-[#7de6c4] bg-[#254d44]",
    "controller-meta": "text-xs text-[#a5b7b3]",
    hint: "mt-[5px] text-[13px] text-[#9faeac]",
    "raw-buttons": "whitespace-nowrap font-mono text-xs text-[#7de6c4]",
    "joycon-stage":
      "flex min-h-[520px] items-center justify-center gap-[72px] px-[14%] py-[18px] max-[720px]:gap-[25px] max-[720px]:p-5",
    "axis-readout": "grid min-w-[194px] gap-[6px] max-[720px]:min-w-[150px]",
    "readout-label": "text-xs tracking-[1px] text-[#9faeac] uppercase",
    "axis-output":
      "mb-5 whitespace-pre-line font-mono text-sm font-600 leading-[1.45] text-[#7de6c4]",
    "imu-output": "mb-3 text-[11px] leading-[1.35] text-[#a9d9cc]",
  },
  safelist: ["connected", "error"],
});
