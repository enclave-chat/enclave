import { ClientMeta, MessageData, StoredMessage } from "@/lib/types";

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
    }
  | {
      method: "Messages";
      messages: Record<string, StoredMessage[]>;
    };

export type ServerMethod =
  | {
      method: "Initialize";
      public_key: string;
      signature: string;
      timestamp: number;
      hostname: string;
    }
  | {
      method: "Error";
      error: string;
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
      method: "GetMessages";
      channel_id: string;
      chunk: number;
    };
