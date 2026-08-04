import assert from "node:assert/strict";
import test from "node:test";
import { createProceduralJoyCon } from "./procedural-joycon.ts";

test("procedural Joy-Cons expose the interactive controls used by the renderer", () => {
  const left = createProceduralJoyCon("left");
  const right = createProceduralJoyCon("right");

  for (const name of ["StickAssembly", "DPad_Up", "Button_L", "Button_ZL", "Button_SL", "Button_SR"]) {
    assert.ok(left.getObjectByName(`SM_JoyCon_left_${name}`), `left ${name}`);
  }
  for (const name of ["StickAssembly", "Button_A", "Button_X", "Button_R", "Button_ZR", "Button_SL", "Button_SR"]) {
    assert.ok(right.getObjectByName(`SM_JoyCon_right_${name}`), `right ${name}`);
  }
});
