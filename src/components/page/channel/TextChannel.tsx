import Enclave from "@/app/app";
import { ChannelPageProps } from "@/components/page/PageView";

export default function TextChannel({
  appRef,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
}) {
  return <div>{appRef.current?.page?.channel.name}</div>;
}
