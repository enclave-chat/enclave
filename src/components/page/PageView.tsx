import Enclave from "@/app/app";
import ChannelPage from "./channel/ChannelPage";
import { Channel } from "@/app/protocol";

export type ChannelPageProps = {
  kind: "channel";
  channel: Channel;
};

export type Page = ChannelPageProps;

export default function Page({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  switch (appRef.current?.page?.kind) {
    case "channel":
      return <ChannelPage appRef={appRef} />;

    default:
      return null;
  }
}
