import joinSfx from "@/assets/join.wav";
import leaveSfx from "@/assets/leave.wav";
import messageSfx from "@/assets/message.mp3";

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

export function playMessage() {
  const audio = new Audio(messageSfx);

  audio.currentTime = 0;
  audio.play();
}
