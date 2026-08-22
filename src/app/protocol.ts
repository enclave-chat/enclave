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
    }
  | {
      method: "Users";
      users: Record<string, ClientMeta>;
    }
  | {
      method: "MessageEdited";
      channel_id: string;
      message: StoredMessage;
    }
  | {
      method: "MessageDeleted";
      channel_id: string;
      message_id: string;
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
    }
  | {
      method: "GetUsers";
      pubkeys: string[];
    }
  | {
      method: "EditMessage";
      message_id: string;
      channel_id: string;
      content: string;
      signature: string;
    }
  | {
      method: "DeleteMessage";
      channel_id: string;
      message_id: string;
    };
