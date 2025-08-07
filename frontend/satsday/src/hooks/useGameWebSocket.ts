import { useEffect, useState } from "react";
import { GameHistoryItem } from "@/services/api";
import {
  gameWebSocketService,
  DonationItem,
  isDonation,
  isGameHistoryArray,
  isGameHistoryItem,
} from "@/services/websocket";

interface UseGameWebSocketReturn {
  activities: GameHistoryItem[];
  donations: DonationItem[];
  isConnected: boolean;
  isLoading: boolean;
  reconnect: () => void;
}

export function useGameWebSocket(
  maxItems: number = 20,
): UseGameWebSocketReturn {
  const [activities, setActivities] = useState<GameHistoryItem[]>([]);
  const [donations, setDonations] = useState<DonationItem[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const unsubscribe = gameWebSocketService.subscribe(
      // Message callback
      (data) => {
        if (isGameHistoryArray(data)) {
          // Initial history load
          setActivities(data.slice(0, maxItems));
          setIsLoading(false);
        } else if (isGameHistoryItem(data)) {
          // Real-time game result - add to beginning of activities list
          setActivities((prev) => [data, ...prev].slice(0, maxItems));
        } else if (isDonation(data)) {
          // Real-time donation - add to beginning of donations list
          setDonations((prev) => [data, ...prev].slice(0, maxItems));
        }
      },
      // Connection status callback
      (connected) => {
        setIsConnected(connected);
        if (!connected && activities.length === 0) {
          setIsLoading(false);
        }
      },
    );

    // Cleanup on unmount
    return () => {
      unsubscribe();
    };
  }, [maxItems]);

  const reconnect = () => {
    gameWebSocketService.reconnect();
  };

  return {
    activities,
    donations,
    isConnected,
    isLoading,
    reconnect,
  };
}
