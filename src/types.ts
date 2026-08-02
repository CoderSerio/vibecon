export type Controller = {
  id: string;
  name: string;
  product_id: number;
  transport: string;
};

export type Stick = {
  x: number;
  y: number;
  normalized_x: number;
  normalized_y: number;
};

export type ImuSample = {
  acceleration: [number, number, number];
  gyroscope: [number, number, number];
};

export type InputReport = {
  report_id: number;
  bytes: number[];
  left_stick: Stick | null;
  right_stick: Stick | null;
  buttons: [number, number, number] | null;
  imu: ImuSample | null;
};

export type LogEntry = {
  device_id: string;
  timestamp: Date;
  previous_report?: InputReport;
  report: InputReport;
};

export type Label = {
  kind: "stick" | "button";
  target: string;
  phase?: "pressed" | "released" | "moved" | "reset";
};

export type MappingAction =
  | "window_previous"
  | "window_next"
  | "focus_codex";

export type MappingBinding = {
  id: string;
  control: string;
  action: MappingAction;
  enabled: boolean;
};

export type MappingPreset = {
  id: string;
  name: string;
  enabled: boolean;
  bindings: MappingBinding[];
};

export type MappingConfig = {
  version: number;
  activePresetId: string;
  presets: MappingPreset[];
};
