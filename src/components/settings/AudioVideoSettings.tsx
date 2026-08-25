import { Volume2 } from "lucide-react";

export function AudioVideoSettings() {
  return (
    <div className="space-y-5 w-full">
      <h2 className="text-lg font-semibold flex items-center gap-2">
        <Volume2 /> Audio & Video
      </h2>
      <p className="text-sm text-muted-foreground">
        Microphone/camera device selection goes here.
      </p>
    </div>
  );
}
