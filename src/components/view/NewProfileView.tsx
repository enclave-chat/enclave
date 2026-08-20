import * as ed from "@noble/ed25519";
import { base58 } from "@scure/base";

import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { saveAccounts } from "@/lib/accounts";
import Enclave from "@/app/app";
import { Avatar, AvatarFallback, AvatarImage } from "../ui/avatar";

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(new Error("Failed to read image"));
    reader.readAsDataURL(file);
  });
}

export function NewProfilePage({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  const [displayName, setDisplayName] = useState("");
  const [avatar, setAvatar] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleAvatarChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;

    if (!file.type.startsWith("image/")) {
      setError("Please select an image file");
      return;
    }

    try {
      const dataUrl = await readFileAsDataUrl(file);
      setAvatar(dataUrl);
      setError(null);
    } catch {
      setError("Failed to load image");
    }
  }

  async function handleCreate() {
    const name = displayName.trim();
    if (!name) {
      setError("Display name is required");
      return;
    }

    setSubmitting(true);
    setError(null);

    try {
      if (!appRef.current) {
        console.error("AppRef is not initialized yet");
        return;
      }

      if (!appRef.current.accounts) {
        appRef.current.accounts = { activeAccount: 0, accounts: [] };
      }

      const { secretKey } = ed.keygen();

      appRef.current.accounts.activeAccount =
        appRef.current.accounts.accounts.length;

      appRef.current.accounts.accounts.push({
        meta: {
          displayName: name,
          avatar: avatar ?? undefined,
        },
        privateKey: base58.encode(secretKey),
      });

      saveAccounts(appRef.current.accounts);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create profile");
    } finally {
      setSubmitting(false);

      window.location.reload();
    }
  }

  return (
    <div className="flex h-screen w-full items-center justify-center">
      <div className="w-full max-w-sm space-y-6">
        <div className="space-y-2 text-center">
          <h1 className="text-2xl font-semibold">Create your profile</h1>
          <p className="text-sm text-muted-foreground">
            Enclave generates a keypair for your identity — no email or password
            required.
          </p>
        </div>

        <div className="flex flex-col items-center gap-3">
          <label htmlFor="avatar-upload" className="cursor-pointer">
            <div className="h-20 w-20 overflow-hidden rounded-full border border-border bg-muted flex items-center justify-center hover:opacity-80 transition-opacity">
              <Avatar className="h-full w-auto aspect-square">
                {avatar && <AvatarImage src={avatar} />}
                <AvatarFallback>{displayName[0] || "J"}</AvatarFallback>
              </Avatar>
            </div>
          </label>
          <input
            id="avatar-upload"
            type="file"
            accept="image/*"
            className="hidden"
            onChange={handleAvatarChange}
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="display-name">Display name</Label>
          <Input
            id="display-name"
            placeholder="e.g. John Doe"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
          />
          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>

        <Button className="w-full" onClick={handleCreate} disabled={submitting}>
          {submitting ? "Creating..." : "Create Profile"}
        </Button>
      </div>
    </div>
  );
}
