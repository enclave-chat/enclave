import joinSfx from "@/assets/join.wav";
import leaveSfx from "@/assets/leave.wav";

export function playJoin() {
  const audio = new Audio(joinSfx);

  audio.currentTime = 0;
  audio.play();
}

export function playLeave() {
  const audio = new Audio(leaveSfx);

  audio.currentTime = 0;
  audio.play();
}
