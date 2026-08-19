import Enclave from "@/app/app";
import { Channel, ChannelKind } from "@/app/protocol";
import { HashIcon } from "lucide-react";

export function ChannelIcon({ kind }: { kind: ChannelKind["kind"] }) {
  switch (kind) {
    case "text":
      return <HashIcon className="p-0.5" />;
    default:
      return null;
  }
}

export function RenderChannels({ channels }: { channels: Channel[] }) {
  return channels.map((channel) =>
    channel.kind === "category" ? (
      <div>
        <div className="w-full px-0.5 py-1 rounded-md select-none cursor-default text-sm text-foreground/70">
          {channel.name}
        </div>
        <div className="pl-2 pr-1.5 pt-0.5 w-full flex flex-col gap-1">
          <RenderChannels channels={channel.channels} />
        </div>
      </div>
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
    appRef.current?.server?.meta && (
      <div className="h-screen">
        <header className="px-3 pt-2.5 pb-2.5 text-lg font-semibold border-b border-b-border">
          <h1>{appRef.current.server.meta.name}</h1>
        </header>

        <section className="px-1.5 pt-3.5 w-full flex flex-col gap-1">
          <RenderChannels channels={appRef.current.server.meta.channels} />
        </section>
      </div>
    )
  );
}
