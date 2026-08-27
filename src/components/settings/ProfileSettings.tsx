import { User } from "lucide-react";

export function ProfileSettings() {
  return (
    <div className="space-y-5 w-full">
      <h2 className="text-lg font-semibold flex items-center gap-2">
        <User /> Profile
      </h2>
      <p className="text-sm text-muted-foreground">
        Display name and avatar settings go here.
      </p>
    </div>
  );
}
