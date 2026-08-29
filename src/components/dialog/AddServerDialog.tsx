import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Button } from "@/components/ui/button";
import { Info, PlusIcon } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "../ui/tooltip";

// hostname[:port][/path] — explicitly no protocol/scheme allowed
const HOSTNAME_PATTERN =
  /^(?!.*:\/\/)[a-zA-Z0-9.-]+(:\d{1,5})?(\/[a-zA-Z0-9\-._~%!$&'()*+,;=:@/]*)?$/;

function validateHostname(value: string): string | null {
  if (!value.trim()) {
    return "Hostname is required";
  }
  if (value.includes("://")) {
    return "Don't include a protocol (e.g. https://)";
  }
  if (!HOSTNAME_PATTERN.test(value)) {
    return "Invalid hostname format";
  }
  return null;
}

type AddServerDialogProps = {
  onAdd: (hostname: string, isSecure: boolean) => Promise<void> | void;
};

export function AddServerDialog({ onAdd }: AddServerDialogProps) {
  const [open, setOpen] = useState(false);
  const [hostname, setHostname] = useState("");
  const [isSecure, setIsSecure] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  function handleHostnameChange(value: string) {
    setHostname(value);
    if (error) {
      setError(validateHostname(value));
    }
  }

  async function handleSubmit() {
    const validationError = validateHostname(hostname);
    if (validationError) {
      setError(validationError);
      return;
    }

    setSubmitting(true);
    try {
      await onAdd(hostname.trim(), isSecure);
      setHostname("");
      setIsSecure(true);
      setError(null);
      setOpen(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to add server");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger
        render={() => (
          <Button
            variant="secondary"
            className="aspect-square w-full h-auto rounded-lg"
            onClick={() => setOpen(true)}
          >
            <PlusIcon />
          </Button>
        )}
      />

      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add a Server</DialogTitle>
          <DialogDescription>
            Enter the server's address to connect.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="hostname">Hostname</Label>
            <Input
              id="hostname"
              placeholder="myserver.example.com:8080"
              value={hostname}
              onChange={(e) => handleHostnameChange(e.target.value)}
              aria-invalid={!!error}
            />
            {error && <p className="text-sm text-destructive">{error}</p>}
          </div>

          <div className="flex items-center justify-between">
            <Tooltip>
              <TooltipTrigger>
                <Label
                  htmlFor="secure-toggle"
                  className="flex items-center gap-1 cursor-help"
                >
                  TLS
                  <Info className="h-3.5 w-3.5 text-muted-foreground" />
                </Label>
              </TooltipTrigger>

              <TooltipContent>
                <p className="max-w-xs">
                  TLS is not recommended because there is a encryption layer
                  already built in to Enclave
                </p>
              </TooltipContent>
            </Tooltip>
            <Switch
              id="secure-toggle"
              checked={isSecure}
              onCheckedChange={setIsSecure}
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={submitting}>
            {submitting ? "Adding..." : "Add"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
