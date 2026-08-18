import * as ed from "@noble/ed25519";
import { base58 } from "@scure/base";

import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { saveAccounts } from "@/lib/accounts";
import Enclave from "@/app/app";

export function NewProfilePage({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  const [displayName, setDisplayName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

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

      const newAccount = {
        displayName: name,
        privateKey: base58.encode(secretKey),
      };

      appRef.current.accounts.activeAccount =
        appRef.current.accounts.accounts.length;

      appRef.current.accounts.accounts.push(newAccount);

      saveAccounts(appRef.current.accounts);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create profile");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="flex h-screen items-center justify-center">
      <div className="w-full max-w-sm space-y-6">
        <div className="space-y-2 text-center">
          <h1 className="text-2xl font-semibold">Create your profile</h1>
          <p className="text-sm text-muted-foreground">
            Enclave generates a keypair for your identity — no email or password
            required.
          </p>
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
