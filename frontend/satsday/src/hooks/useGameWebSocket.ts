import { useEffect, useState } from "react";
import { GameHistoryItem } from "@/services/api";
import { gameWebSocketService } from "@/services/websocket";

interface UseGameWebSocketReturn {
  activities: GameHistoryItem[];
  isConnected: boolean;
  isLoading: boolean;
  reconnect: () => void;
}

export function useGameWebSocket(
  maxItems: number = 20,
): UseGameWebSocketReturn {
  const [activities, setActivities] = useState<GameHistoryItem[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const unsubscribe = gameWebSocketService.subscribe(
      // Message callback
      (data) => {
        if (Array.isArray(data)) {
          // Initial history load
          setActivities(data.slice(0, maxItems));
          setIsLoading(false);
        } else {
          // Real-time update - add to beginning of list
          setActivities((prev) => [data, ...prev].slice(0, maxItems));
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
    isConnected,
    isLoading,
    reconnect,
  };
}
