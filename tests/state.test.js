import { test } from "node:test";
import assert from "node:assert/strict";
import { blockers, nextStep, parseProgress } from "../src/state.js";

const ready = {
  spotifyPath: "C:\\Spotify",
  prefsFound: true,
  spotifyRunning: false,
  admin: false,
};

test("installation needs a non-elevated account, a signed-in desktop client, and Spotify closed", () => {
  assert.deepEqual(blockers(ready), []);
  assert.equal(blockers(undefined).length, 1);
  for (const status of [
    { admin: true },
    { spotifyPath: null },
    { prefsFound: false },
    { spotifyRunning: true },
  ]) {
    assert.equal(blockers({ ...ready, ...status }).length, 1);
  }
});

test("wizard requires ready checks, keeps busy screens in place, and cannot skip installation", () => {
  assert.equal(nextStep(0, undefined, false), 0);
  assert.equal(nextStep(0, { ...ready, spotifyRunning: true }, false), 0);
  assert.equal(nextStep(0, ready, true), 0);
  assert.equal(nextStep(0, ready, false), 1);
  assert.equal(nextStep(1, ready, true), 1);
  assert.equal(nextStep(1, ready, false), 2);
  assert.equal(nextStep(2, ready, false), 2);
  assert.equal(nextStep(3, ready, false), 3);
});

test("only real installer stage markers advance the progress UI", () => {
  assert.deepEqual(parseProgress("[step:3] Backing up Spotify"), {
    step: 3,
    title: "Backing up Spotify",
  });
  for (const line of [
    "[step:0] invalid",
    "[step:5] invalid",
    "Downloaded 30%",
    "prefix [step:2] nope",
    "[step:2] ",
  ]) {
    assert.equal(parseProgress(line), null);
  }
});
