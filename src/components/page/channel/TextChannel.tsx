import Enclave from "@/app/app";
import { ChannelPageProps } from "@/components/page/PageView";
import { useEffect } from "react";

export default function TextChannel({
  appRef,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
}) {
  const channel = appRef.current?.page?.channel;

  if (!channel) return null;

  useEffect(() => {
    appRef.current?.server?.websocket?.send({
      method: "GetMessages",
      channel_id: channel.id,
      chunk: 0,
    });

    appRef.current?.forceRender();
  }, []);

  return (
    <div>{JSON.stringify(appRef.current?.server?.messages[channel.id])}</div>
  );
}
