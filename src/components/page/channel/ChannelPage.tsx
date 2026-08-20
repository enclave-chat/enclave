import Enclave from "@/app/app";
import { ChannelPageProps } from "@/components/page/PageView";

export default function ChannelPage({
  appRef,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
}) {
  switch (appRef.current?.page?.channel.kind) {
    case "text":
      return <ChannelPage appRef={appRef} />;

    default:
      return null;
  }
}
