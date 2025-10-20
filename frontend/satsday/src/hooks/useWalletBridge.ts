import { useState, useEffect, useRef } from "react";
import { LendasatClient } from "@bonomat/satsday-wallet-bridge";

/**
 * Hook to detect if app is running in an iframe with a functional wallet bridge
 * @returns Object containing bridge client and availability status
 */
export function useWalletBridge() {
  const [isAvailable, setIsAvailable] = useState(false);
  const [isChecking, setIsChecking] = useState(true);
  const clientRef = useRef<LendasatClient | null>(null);

  useEffect(() => {
    const checkBridgeAvailability = async () => {
      // Check if we're running in an iframe
      const inIframe = window.self !== window.top;

      if (!inIframe) {
        console.log("[WalletBridge] Not running in an iframe");
        setIsChecking(false);
        return;
      }

      // Initialize the bridge client
      const client = new LendasatClient(2000); // 2 second timeout for initial check
      clientRef.current = client;
      console.log("[WalletBridge] Client initialized, checking if parent implements bridge...");

      try {
        // Try to get an address to verify the bridge is functional
        // This will timeout if the parent doesn't implement the bridge
        const address = await client.getAddress();

        if (address) {
          console.log("[WalletBridge] Bridge is functional, address received:", address);
          setIsAvailable(true);
        } else {
          console.log("[WalletBridge] Bridge responded but no address available");
          setIsAvailable(false);
        }
      } catch (error) {
        console.log("[WalletBridge] Bridge not available or not implemented by parent:", error);
        setIsAvailable(false);
        // Clean up the client if bridge is not available
        client.destroy();
        clientRef.current = null;
      } finally {
        setIsChecking(false);
      }
    };

    checkBridgeAvailability();

    return () => {
      // Cleanup on unmount
      if (clientRef.current) {
        clientRef.current.destroy();
        clientRef.current = null;
      }
    };
  }, []);

  return {
    isAvailable,
    isChecking,
    client: clientRef.current,
  };
}
