import Enclave from "@/app/app";
import { ChannelPageProps } from "@/components/page/PageView";
import TextChannel from "./TextChannel";

export default function ChannelPage({
  appRef,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
}) {
  if (!appRef.current?.page?.channel.kind) return null;

  switch (appRef.current?.page?.channel.kind) {
    case "text":
      return <TextChannel appRef={appRef} />;
  }
}
