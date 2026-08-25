import { useEffect, useState } from "react";
import { Volume2, Mic, Speaker } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { getConfig, updateConfig, saveConfig, Config } from "@/lib/config";

export function AudioVideoSettings() {
  const [config, setConfig] = useState<Config | null>(null);
  const [inputDevices, setInputDevices] = useState<string[]>([]);
  const [outputDevices, setOutputDevices] = useState<string[]>([]);

  useEffect(() => {
    getConfig().then(setConfig);
    invoke<string[]>("list_input_devices").then(setInputDevices);
    invoke<string[]>("list_output_devices").then(setOutputDevices);
  }, []);

  async function handleChange(patch: Partial<Config>) {
    if (!config) return;
    const updated = { ...config, ...patch };
    setConfig(updated);
    await updateConfig(updated);
    await saveConfig();
  }

  if (!config) return null;

  return (
    <div className="space-y-8 w-full">
      <h2 className="text-lg font-semibold flex items-center gap-2">
        <Volume2 /> Audio & Video
      </h2>

      <div className="space-y-4">
        <div className="flex items-center gap-2 text-sm font-medium">
          <Mic className="h-4 w-4" />
          Input
        </div>

        <div className="space-y-2">
          <Label>Input device</Label>
          <Select
            value={config.inputDeviceName ?? undefined}
            onValueChange={(value) => handleChange({ inputDeviceName: value })}
          >
            <SelectTrigger className="w-full">
              <SelectValue placeholder="Select a microphone" />
            </SelectTrigger>
            <SelectContent>
              {inputDevices.map((device) => (
                <SelectItem key={device} value={device}>
                  {device}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label>Input volume</Label>
            <span className="text-xs text-muted-foreground">
              {config.inputVolume}%
            </span>
          </div>
          <Slider
            value={[config.inputVolume]}
            onValueChange={(value) =>
              handleChange({ inputVolume: value as any })
            }
            max={100}
            step={1}
          />
        </div>
      </div>

      <div className="space-y-4">
        <div className="flex items-center gap-2 text-sm font-medium">
          <Speaker className="h-4 w-4" />
          Output
        </div>

        <div className="space-y-2">
          <Label>Output device</Label>
          <Select
            value={config.outputDeviceName ?? undefined}
            onValueChange={(value) => handleChange({ outputDeviceName: value })}
          >
            <SelectTrigger className="w-full">
              <SelectValue placeholder="Select a speaker" />
            </SelectTrigger>
            <SelectContent>
              {outputDevices.map((device) => (
                <SelectItem key={device} value={device}>
                  {device}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label>Output volume</Label>
            <span className="text-xs text-muted-foreground">
              {config.outputVolume}%
            </span>
          </div>
          <Slider
            value={[config.outputVolume]}
            onValueChange={(value) =>
              handleChange({ outputVolume: value as any })
            }
            max={100}
            step={1}
          />
        </div>
      </div>
    </div>
  );
}
