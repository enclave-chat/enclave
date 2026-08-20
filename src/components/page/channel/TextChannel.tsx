import Enclave from "@/app/app";
import { ChannelPageProps } from "@/components/page/PageView";

export default function TextChannel({
  appRef,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
}) {
  const channel = appRef.current?.page?.channel;

  if (!channel) return null;

  return (
    <div
      onClick={() => {
        appRef.current?.sendMessage("Hello, World", channel.id);
      }}
    >
      {channel.name}
    </div>
  );
}
