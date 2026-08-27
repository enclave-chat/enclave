import { Palette } from "lucide-react";

export function AppearanceSettings() {
  return (
    <div className="space-y-5 w-full">
      <h2 className="text-lg font-semibold flex items-center gap-2">
        <Palette /> Appearance
      </h2>
      <p className="text-sm text-muted-foreground">
        Theme and display settings go here.
      </p>
    </div>
  );
}
