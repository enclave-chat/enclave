import Enclave from "@/app/app";
import ServerSidebar from "./ServerSidebar";
import MainPageSidebar from "./MainPageSidebar";

export default function Sidebar({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  switch (appRef.current?.sidebarPageKind) {
    case "main":
      return <MainPageSidebar appRef={appRef} />;
    case "server":
      return <ServerSidebar appRef={appRef} />;
  }

  return <MainPageSidebar appRef={appRef} />;
}
