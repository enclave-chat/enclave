import { ClientMeta, MessageData } from "@/lib/types";

export type ClientMethod =
  | {
      method: "Initialized";
      public_key: string;
      signature: string;
      timestamp: number;
      hostname: string;
    }
  | {
      method: "Error";
      error: string;
    };

export type ServerMethod =
  | {
      method: "Initialize";
      public_key: string;
      signature: string;
      timestamp: number;
      hostname: string;
    }
  | ({
      method: "Meta";
    } & ClientMeta)
  | {
      method: "SendMessage";
      channel_id: string;
      data: MessageData;
    }
  | {
      method: "Error";
      error: string;
    };
