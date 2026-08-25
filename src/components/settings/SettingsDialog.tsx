import { useState } from "react";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { User, Palette, Mic, Volume2 } from "lucide-react";
import { ProfileSettings } from "./ProfileSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { AudioVideoSettings } from "./AudioVideoSettings";
import Enclave from "@/app/app";

type SettingsCategory = "profile" | "appearance" | "audio-video";

const CATEGORIES: {
  id: SettingsCategory;
  label: string;
  icon: React.ElementType;
}[] = [
  { id: "profile", label: "Profile", icon: User },
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "audio-video", label: "Audio & Video", icon: Volume2 },
];

export function SettingsDialog({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  const [category, setCategory] = useState<SettingsCategory>("profile");

  return (
    <Dialog
      open={appRef.current?.isSettingsOpen}
      onOpenChange={(open) => {
        if (!appRef.current) return;

        appRef.current.isSettingsOpen = open;
        appRef.current.forceRender();
      }}
    >
      <DialogContent
        className="flex h-[85vh] w-[90vw] max-w-5xl gap-0 overflow-hidden p-0 sm:max-w-5xl"
        showCloseButton
      >
        <div className="flex w-56 shrink-0 flex-col border-r bg-muted/30 p-3">
          <span className="mb-2 px-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Settings
          </span>
          <nav className="flex flex-col gap-0.5">
            {CATEGORIES.map(({ id, label, icon: Icon }) => (
              <button
                key={id}
                onClick={() => setCategory(id)}
                className={cn(
                  "flex items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors",
                  category === id
                    ? "bg-accent text-accent-foreground"
                    : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
                )}
              >
                <Icon className="h-4 w-4" />
                {label}
              </button>
            ))}
          </nav>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          {category === "profile" && <ProfileSettings />}
          {category === "appearance" && <AppearanceSettings />}
          {category === "audio-video" && <AudioVideoSettings />}
        </div>
      </DialogContent>
    </Dialog>
  );
}
