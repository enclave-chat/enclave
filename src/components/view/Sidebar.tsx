import Enclave from "@/app/app";
import { Channel, ChannelKind } from "@/app/protocol";
import { ChevronDown, ChevronUp, HashIcon } from "lucide-react";
import { useState } from "react";

export function ChannelIcon({ kind }: { kind: ChannelKind["kind"] }) {
  switch (kind) {
    case "text":
      return <HashIcon className="p-0.5" />;
    default:
      return null;
  }
}

export function RenderCategory({ channel }: { channel: Channel }) {
  if (channel.kind !== "category") return null;

  const [open, setOpen] = useState(true);

  return (
    <div>
      <div
        className="w-full px-2 py-1 rounded-md select-none cursor-default text-sm text-foreground/70 flex items-center"
        onClick={() => setOpen((v) => !v)}
      >
        {channel.name}
        {open ? (
          <ChevronDown className="p-1 shrink" />
        ) : (
          <ChevronUp className="p-1 shrink" />
        )}
      </div>
      {open && (
        <div className="pt-0.5 w-full flex flex-col gap-1">
          <RenderChannels channels={channel.channels} />
        </div>
      )}
    </div>
  );
}

export function RenderChannels({ channels }: { channels: Channel[] }) {
  return channels.map((channel) =>
    channel.kind === "category" ? (
      <RenderCategory channel={channel} />
    ) : (
      <div className="w-full hover:bg-accent px-2.5 py-1.5 rounded-md flex gap-2.5 items-center select-none cursor-default text-sm text-foreground/70">
        <ChannelIcon kind={channel.kind} />
        {channel.name}
      </div>
    ),
  );
}

export default function Sidebar({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  return (
    <div className="h-screen relative w-full @container">
      {appRef.current?.server?.meta && (
        <>
          <header className="px-3 pt-2.5 pb-2.5 text-lg font-semibold border-b border-b-border">
            <h1>{appRef.current.server.meta.name}</h1>
          </header>

          <section className="px-1.5 pt-3.5 w-full flex flex-col gap-1">
            <RenderChannels channels={appRef.current.server.meta.channels} />
          </section>
        </>
      )}

      <div className="absolute -left-16 right-16 bottom-0 h-24 w-[calc(100%+4rem-2rem)] z-10 ml-4 mb-4 @max-[150px]:hidden" />
    </div>
  );
}
