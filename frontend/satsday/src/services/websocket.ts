import { GameHistoryItem } from "./api";

export interface DonationItem {
  id: string;
  amount: number;
  sender: string;
  input_tx_id: string;
  timestamp: number;
}

// Legacy websocket message format (for history)
export interface LegacyWebSocketMessage {
  type: "history";
  games: GameHistoryItem[];
}

// New backend websocket message format
export interface BackendWebSocketMessage {
  type: "game_result";
  id: string;
  amount_sent: number;
  multiplier: number;
  result_number: number;
  target_number: number;
  is_win: boolean;
  payout?: number;
  input_tx_id: string;
  output_tx_id: string | null;
  nonce?: string;
  nonce_hash: string;
  timestamp: number;
}

export interface DonationWebSocketMessage {
  type: "donation";
  id: string;
  amount: number;
  sender: string;
  input_tx_id: string;
  timestamp: number;
}

export type WebSocketMessage =
  | LegacyWebSocketMessage
  | BackendWebSocketMessage
  | DonationWebSocketMessage;

export type WebSocketCallback = (
  data: GameHistoryItem | GameHistoryItem[] | DonationItem,
) => void;
export type ConnectionCallback = (connected: boolean) => void;

class GameWebSocketService {
  private ws: WebSocket | null = null;
  private reconnectTimeout: NodeJS.Timeout | null = null;
  private messageCallbacks: Set<WebSocketCallback> = new Set();
  private connectionCallbacks: Set<ConnectionCallback> = new Set();
  private reconnectDelay = 5000;
  private shouldReconnect = true;
  private cachedHistory: GameHistoryItem[] | null = null;

  constructor() {
    this.connect();
  }

  private getWebSocketUrl(): string {
    const API_BASE_URL =
      import.meta.env.VITE_API_BASE_URL || window.location.origin;

    // Convert http/https to ws/wss
    let wsUrl = API_BASE_URL.replace(/^https:/, "wss:").replace(
      /^http:/,
      "ws:",
    );

    // Add /ws endpoint
    wsUrl = wsUrl.endsWith("/") ? `${wsUrl}ws` : `${wsUrl}/ws`;

    return wsUrl;
  }

  private connect(): void {
    if (
      this.ws?.readyState === WebSocket.OPEN ||
      this.ws?.readyState === WebSocket.CONNECTING
    ) {
      return;
    }

    // Clean up any existing connection
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    try {
      const wsUrl = this.getWebSocketUrl();

      this.ws = new WebSocket(wsUrl);

      this.ws.onopen = () => {
        this.notifyConnectionStatus(true);
      };

      this.ws.onmessage = (event) => {
        try {
          const data: WebSocketMessage = JSON.parse(event.data);

          if (data.type === "history") {
            // Initial history load
            this.cachedHistory = data.games;
            this.notifySubscribers(data.games);
          } else if (data.type === "game_result") {
            // Real-time game result - convert to GameHistoryItem format
            const gameItem: GameHistoryItem = {
              id: data.id,
              amount_sent: data.amount_sent,
              multiplier: data.multiplier,
              result_number: data.result_number,
              target_number: data.target_number,
              is_win: data.is_win,
              payout: data.payout,
              input_tx_id: data.input_tx_id,
              output_tx_id: data.output_tx_id,
              nonce: data.nonce,
              nonce_hash: data.nonce_hash,
              timestamp: data.timestamp,
            };
            this.notifySubscribers(gameItem);
          } else if (data.type === "donation") {
            // Real-time donation notification
            const donationItem: DonationItem = {
              id: data.id,
              amount: data.amount,
              sender: data.sender,
              input_tx_id: data.input_tx_id,
              timestamp: data.timestamp,
            };
            this.notifySubscribers(donationItem);
          }
        } catch (error) {
          console.error("Failed to parse WebSocket message:", error);
        }
      };

      this.ws.onerror = (error) => {
        console.error("WebSocket error:", error);
      };

      this.ws.onclose = () => {
        console.log("WebSocket disconnected");
        this.notifyConnectionStatus(false);

        if (this.shouldReconnect) {
          // Reconnect after delay
          this.reconnectTimeout = setTimeout(() => {
            this.connect();
          }, this.reconnectDelay);
        }
      };
    } catch (error) {
      console.error("Failed to connect WebSocket:", error);
      this.notifyConnectionStatus(false);
    }
  }

  private notifySubscribers(
    data: GameHistoryItem | GameHistoryItem[] | DonationItem,
  ): void {
    this.messageCallbacks.forEach((callback) => {
      callback(data);
    });
  }

  private notifyConnectionStatus(connected: boolean): void {
    this.connectionCallbacks.forEach((callback) => {
      callback(connected);
    });
  }

  public subscribe(
    onMessage: WebSocketCallback,
    onConnectionChange?: ConnectionCallback,
  ): () => void {
    this.messageCallbacks.add(onMessage);

    if (onConnectionChange) {
      this.connectionCallbacks.add(onConnectionChange);
      // Notify current connection status
      onConnectionChange(this.ws?.readyState === WebSocket.OPEN);
    }

    // If we have cached history data and this is the first subscriber, send it immediately
    if (this.cachedHistory && this.messageCallbacks.size === 1) {
      setTimeout(() => {
        if (this.cachedHistory) {
          onMessage(this.cachedHistory);
          this.cachedHistory = null; // Clear cache after sending
        }
      }, 0);
    }

    // Return unsubscribe function
    return () => {
      this.messageCallbacks.delete(onMessage);
      if (onConnectionChange) {
        this.connectionCallbacks.delete(onConnectionChange);
      }
    };
  }

  public disconnect(): void {
    this.shouldReconnect = false;

    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  public reconnect(): void {
    this.shouldReconnect = true;
    this.disconnect();
    this.connect();
  }

  public isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
}

// Helper functions to distinguish message types
export function isDonation(
  data: GameHistoryItem | GameHistoryItem[] | DonationItem,
): data is DonationItem {
  return !Array.isArray(data) && "sender" in data && !("multiplier" in data);
}

export function isGameHistoryArray(
  data: GameHistoryItem | GameHistoryItem[] | DonationItem,
): data is GameHistoryItem[] {
  return Array.isArray(data);
}

export function isGameHistoryItem(
  data: GameHistoryItem | GameHistoryItem[] | DonationItem,
): data is GameHistoryItem {
  return !Array.isArray(data) && "multiplier" in data;
}

// Create singleton instance
export const gameWebSocketService = new GameWebSocketService();
