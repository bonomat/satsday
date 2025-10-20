import type {
  WalletRequest,
  WalletResponse,
  ErrorResponse,
  PaymentReceivedNotification,
} from "./types";
import { isWalletResponse, isWalletNotification } from "./types";

/**
 * Client for Lendasat iframe to communicate with parent wallet
 *
 * Usage:
 * ```typescript
 * const client = new LendasatClient();
 * const capabilities = await client.getCapabilities();
 * const publicKey = await client.getPublicKey();
 * const path = await client.getDerivationPath();
 * const npub = await client.getNpub();
 * const signed = await client.signPsbt(psbtBase64);
 * ```
 */
export class LendasatClient {
  private pendingRequests: Map<
    string,
    { resolve: (value: unknown) => void; reject: (reason: Error) => void }
  >;
  private targetOrigin: string;
  private messageHandler: ((event: MessageEvent) => void) | null = null;
  private paymentReceivedCallback:
    | ((notification: PaymentReceivedNotification) => void)
    | null = null;

  /**
   * @param targetOrigin - The origin of the parent wallet (default: "*" for development, should be specific in production)
   * @param timeout - Request timeout in milliseconds (default: 30000)
   */
  constructor(
    private readonly timeout: number = 30000,
    targetOrigin: string = "*",
  ) {
    this.pendingRequests = new Map();
    this.targetOrigin = targetOrigin;
    this.setupMessageListener();
  }

  private setupMessageListener(): void {
    this.messageHandler = (event: MessageEvent) => {
      // TODO: In production, validate event.origin matches expected parent origin
      const message = event.data;

      // Handle notifications (no response expected)
      if (isWalletNotification(message)) {
        console.log(
          "[WalletBridge Client] Received notification:",
          message.type,
          message,
        );

        if (message.type === "PAYMENT_RECEIVED" && this.paymentReceivedCallback) {
          this.paymentReceivedCallback(message);
        }
        return;
      }

      // Handle responses to requests
      if (!isWalletResponse(message)) {
        return;
      }

      console.log(
        "[WalletBridge Client] Received response:",
        message.type,
        message,
      );

      const pending = this.pendingRequests.get(message.id);
      if (!pending) {
        console.warn(
          "[WalletBridge Client] No pending request found for ID:",
          message.id,
        );
        return;
      }

      this.pendingRequests.delete(message.id);

      if (message.type === "ERROR") {
        pending.reject(new Error((message as ErrorResponse).error));
      } else {
        pending.resolve(message);
      }
    };

    window.addEventListener("message", this.messageHandler);
    console.log("[WalletBridge Client] Message listener set up");
  }

  private sendRequest<T extends WalletResponse>(
    request: WalletRequest,
  ): Promise<T> {
    return new Promise((resolve, reject) => {
      const timeoutId = setTimeout(() => {
        this.pendingRequests.delete(request.id);
        reject(
          new Error(
            `Request ${request.type} timed out after ${this.timeout}ms`,
          ),
        );
      }, this.timeout);

      this.pendingRequests.set(request.id, {
        resolve: (value) => {
          clearTimeout(timeoutId);
          resolve(value as T);
        },
        reject: (error) => {
          clearTimeout(timeoutId);
          reject(error);
        },
      });

      if (!window.parent) {
        reject(new Error("Not running in an iframe"));
        return;
      }

      console.log(
        "[WalletBridge Client] Sending request:",
        request.type,
        "to origin:",
        this.targetOrigin,
        request,
      );
      window.parent.postMessage(request, this.targetOrigin);
    });
  }

  private generateId(): string {
    return `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
  }

  /**
   * Get an address from the parent wallet
   * @returns The requested address
   */
  async getAddress(): Promise<string> {
    const response = await this.sendRequest<{
      type: "ADDRESS_RESPONSE";
      id: string;
      address: string;
    }>({
      type: "GET_ADDRESS",
      id: this.generateId(),
    });
    return response.address;
  }

  /**
   * Send funds to an address
   * @param address - Address to send to
   * @param amount - Amount to send in satoshis (for Bitcoin) or smallest unit for other assets
   * @returns Transaction ID (txid) of the broadcast transaction
   */
  async sendToAddress(address: string, amount: number): Promise<string> {
    const response = await this.sendRequest<{
      type: "SEND_TO_ADDRESS_RESPONSE";
      id: string;
      txid: string;
    }>({
      type: "SEND_TO_ADDRESS",
      id: this.generateId(),
      address,
      amount,
    });
    return response.txid;
  }

  /**
   * Register a callback to be called when a payment is received
   * @param callback - Function to call when payment is received
   */
  onPaymentReceived(
    callback: (notification: PaymentReceivedNotification) => void,
  ): void {
    this.paymentReceivedCallback = callback;
    console.log("[WalletBridge Client] Payment received callback registered");
  }

  /**
   * Clean up event listeners
   */
  destroy(): void {
    if (this.messageHandler) {
      window.removeEventListener("message", this.messageHandler);
      this.messageHandler = null;
    }
    this.pendingRequests.clear();
    this.paymentReceivedCallback = null;
  }
}
