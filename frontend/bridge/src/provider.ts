import type { WalletRequest, WalletResponse } from "./types";
import { isWalletRequest } from "./types";

/**
 * Handler functions that the parent wallet must implement
 */
export interface WalletHandlers {
  /**
   * Return an address based on the requested type
   * @returns The requested address, or null if the address type is not supported
   */
  onGetAddress: () => Promise<string | null>;

  /**
   * Send funds to an address
   * @param address - Address to send to
   * @param amount - Amount to send in satoshis (for Bitcoin) or smallest unit for other assets
   * @returns Transaction ID (txid) of the broadcast transaction
   */
  onSendToAddress: (address: string, amount: number) => Promise<string>;
}

/**
 * Provider for parent wallet to handle requests from Lendasat iframe
 *
 * Usage:
 * ```typescript
 * const provider = new WalletProvider({
 *   capabilities: () => ({
 *     bitcoin: { signPsbt: true, sendBitcoin: false },
 *     loanAssets: { supportedAssets: [], canReceive: false, canSend: false },
 *     nostr: { hasNpub: false },
 *     ark: { canSend: true, canReceive: true },
 *   }),
 *   onGetPublicKey: () => keyPair.publicKey.toString('hex'),
 *   onGetDerivationPath: () => "m/84'/0'/0'/0/0",
 *   onGetNpub: () => convertToNpub(keyPair.publicKey),
 *   onSignPsbt: (psbt) => signPsbtWithKey(psbt, keyPair),
 * });
 *
 * // Start listening to messages from the iframe
 * provider.listen(iframeElement);
 * ```
 */
export class WalletProvider {
  private handlers: WalletHandlers;
  private messageHandler: ((event: MessageEvent) => void) | null = null;
  private allowedOrigins: string[];

  /**
   * @param handlers - Handler functions for wallet operations
   * @param allowedOrigins - List of allowed iframe origins (default: ["*"] for development, should be specific in production)
   */
  constructor(handlers: WalletHandlers, allowedOrigins: string[] = ["*"]) {
    this.handlers = handlers;
    this.allowedOrigins = allowedOrigins;
  }

  /**
   * Start listening to messages from the iframe
   * @param iframe - The iframe element to listen to (optional, if not provided will listen to all messages)
   */
  listen(iframe?: HTMLIFrameElement): void {
    if (this.messageHandler) {
      // Already listening
      return;
    }

    this.messageHandler = async (event: MessageEvent) => {
      // Validate origin
      if (
        !this.allowedOrigins.includes("*") &&
        !this.allowedOrigins.includes(event.origin)
      ) {
        console.warn(
          `[WalletBridge Provider] Ignored message from unauthorized origin: ${event.origin}`,
        );
        return;
      }

      // Validate it's from the iframe we're listening to
      if (iframe && event.source !== iframe.contentWindow) {
        return;
      }

      const message = event.data;

      if (!isWalletRequest(message)) {
        return;
      }

      console.log(
        "[WalletBridge Provider] Received request:",
        message.type,
        message,
      );
      await this.handleRequest(message, event.source as Window);
    };

    window.addEventListener("message", this.messageHandler);
  }

  private async handleRequest(
    request: WalletRequest,
    source: Window,
  ): Promise<void> {
    try {
      let response: WalletResponse;

      switch (request.type) {
        case "GET_ADDRESS": {
          const address = await this.handlers.onGetAddress();
          response = {
            type: "ADDRESS_RESPONSE",
            id: request.id,
            address,
          };
          break;
        }

        case "SEND_TO_ADDRESS": {
          const txid = await this.handlers.onSendToAddress(
            request.address,
            request.amount,
          );
          response = {
            type: "SEND_TO_ADDRESS_RESPONSE",
            id: request.id,
            txid,
          };
          break;
        }

        default: {
          throw new Error(`Unhandled request type: ${request}`);
        }
      }

      console.log(
        "[WalletBridge Provider] Sending response:",
        response.type,
        response,
      );
      source.postMessage(response, "*");
    } catch (error) {
      const errorResponse: WalletResponse = {
        type: "ERROR",
        id: request.id,
        error: error instanceof Error ? error.message : String(error),
      };
      console.error("[WalletBridge Provider] Error handling request:", error);
      console.log(
        "[WalletBridge Provider] Sending error response:",
        errorResponse,
      );
      source.postMessage(errorResponse, "*");
    }
  }

  /**
   * Stop listening to messages and clean up
   */
  destroy(): void {
    if (this.messageHandler) {
      window.removeEventListener("message", this.messageHandler);
      this.messageHandler = null;
    }
  }
}
