import { useEffect, useState, useCallback, useRef } from "react";
import useWebSocket, { ReadyState } from "react-use-websocket";
import { GameHistoryItem } from "@/services/api";

export interface DonationItem {
  id: string;
  amount: number;
  sender: string;
  input_tx_id: string;
  timestamp: number;
}

// Legacy websocket message format (for history)
interface LegacyWebSocketMessage {
  type: "history";
  games: GameHistoryItem[];
}

// New backend websocket message format
interface BackendWebSocketMessage {
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

interface DonationWebSocketMessage {
  type: "donation";
  id: string;
  amount: number;
  sender: string;
  input_tx_id: string;
  timestamp: number;
}

type WebSocketMessage =
  | LegacyWebSocketMessage
  | BackendWebSocketMessage
  | DonationWebSocketMessage;

interface UseGameWebSocketReturn {
  activities: GameHistoryItem[];
  donations: DonationItem[];
  isConnected: boolean;
  isLoading: boolean;
  reconnect: () => void;
}

interface UseGameWebSocketOptions {
  onNewGameResult?: (game: GameHistoryItem) => void;
  onNewDonation?: (donation: DonationItem) => void;
}

export function useGameWebSocket(
  maxItems: number = 20,
  options?: UseGameWebSocketOptions,
): UseGameWebSocketReturn {
  const [activities, setActivities] = useState<GameHistoryItem[]>([]);
  const [donations, setDonations] = useState<DonationItem[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const didUnmount = useRef(false);

  // Store callbacks in refs to avoid dependency issues
  const onNewGameResultRef = useRef(options?.onNewGameResult);
  const onNewDonationRef = useRef(options?.onNewDonation);

  // Update refs when callbacks change
  useEffect(() => {
    onNewGameResultRef.current = options?.onNewGameResult;
    onNewDonationRef.current = options?.onNewDonation;
  }, [options?.onNewGameResult, options?.onNewDonation]);

  // Get WebSocket URL
  const getWebSocketUrl = useCallback(() => {
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
  }, []);

  const { lastMessage, readyState, getWebSocket } = useWebSocket(
    getWebSocketUrl(),
    {
      // Reconnect options
      shouldReconnect: () => !didUnmount.current,
      reconnectAttempts: 10,
      reconnectInterval: (attemptNumber) =>
        Math.min(1000 * Math.pow(2, attemptNumber), 30000), // Exponential backoff, max 30s

      // Share the WebSocket connection across components
      share: true,

      // Handle disconnection
      onClose: () => {
        console.log("WebSocket disconnected");
      },

      onError: (error) => {
        console.error("WebSocket error:", error);
      },

      onOpen: () => {
        console.log("WebSocket connected");
      },
    },
  );

  // Process incoming messages
  useEffect(() => {
    if (lastMessage !== null) {
      try {
        const data: WebSocketMessage = JSON.parse(lastMessage.data);

        if (data.type === "history") {
          // Initial history load - don't trigger callbacks
          setActivities(data.games.slice(0, maxItems));
          setIsLoading(false);
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

          // Prepend to activities and limit to maxItems
          setActivities((prev) =>
            [gameItem, ...prev.filter((g) => g.id !== gameItem.id)].slice(
              0,
              maxItems,
            ),
          );

          // Notify subscriber of new game result
          onNewGameResultRef.current?.(gameItem);
        } else if (data.type === "donation") {
          // Real-time donation notification
          const donationItem: DonationItem = {
            id: data.id,
            amount: data.amount,
            sender: data.sender,
            input_tx_id: data.input_tx_id,
            timestamp: data.timestamp,
          };

          // Prepend to donations and limit to maxItems
          setDonations((prev) =>
            [
              donationItem,
              ...prev.filter((d) => d.id !== donationItem.id),
            ].slice(0, maxItems),
          );

          // Notify subscriber of new donation
          onNewDonationRef.current?.(donationItem);
        }
      } catch (error) {
        console.error("Failed to parse WebSocket message:", error);
      }
    }
  }, [lastMessage, maxItems]);

  // Update loading state based on connection
  useEffect(() => {
    if (readyState === ReadyState.OPEN || readyState === ReadyState.CLOSED) {
      setIsLoading(false);
    }
  }, [readyState]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      didUnmount.current = true;
    };
  }, []);

  const isConnected = readyState === ReadyState.OPEN;

  const reconnect = useCallback(() => {
    const ws = getWebSocket();
    if (ws) {
      ws.close();
      // The hook will automatically reconnect due to shouldReconnect
    }
  }, [getWebSocket]);

  return {
    activities,
    donations,
    isConnected,
    isLoading,
    reconnect,
  };
}

// Helper functions for backward compatibility
export function isDonation(
  data: GameHistoryItem | DonationItem,
): data is DonationItem {
  return "sender" in data && !("multiplier" in data);
}

export function isGameHistoryItem(
  data: GameHistoryItem | DonationItem,
): data is GameHistoryItem {
  return "multiplier" in data;
}
