/**
 * Message types for communication between Satsday iframe and parent wallet
 */

export interface GetAddressRequest {
  type: "GET_ADDRESS";
  id: string;
}

export interface SendToAddressRequest {
  type: "SEND_TO_ADDRESS";
  id: string;
  /** Ark address to send to */
  address: string;
  /** Amount to send in satoshis (for Bitcoin) or smallest unit for other assets */
  amount: number;
}

// Request messages sent from iframe to parent wallet
export type WalletRequest = GetAddressRequest | SendToAddressRequest;

// Response messages sent from parent wallet to iframe
export type WalletResponse =
  | AddressResponse
  | SendToAddressResponse
  | ErrorResponse;

export interface AddressResponse {
  type: "ADDRESS_RESPONSE";
  id: string;
  /** The requested address, or null if not supported */
  address: string | null;
}

export interface SendToAddressResponse {
  type: "SEND_TO_ADDRESS_RESPONSE";
  id: string;
  /** Transaction ID (txid) of the broadcast transaction */
  txid: string;
}

export interface ErrorResponse {
  type: "ERROR";
  id: string;
  error: string;
}

// Notification messages sent from parent wallet to iframe (no response expected)
export interface PaymentReceivedNotification {
  type: "PAYMENT_RECEIVED";
  /** Optional address where payment was received */
  address?: string;
  /** Amount received in satoshis */
  amount: number;
  /** Transaction ID */
  txid: string;
  /** Timestamp when the payment was created (Unix timestamp in milliseconds) */
  timestamp: number;
  /** Unix timestamp when the payment was created (milliseconds) */
  createdAt: number;
}

export type WalletNotification = PaymentReceivedNotification;

// Type guards
export function isWalletRequest(message: unknown): message is WalletRequest {
  if (typeof message !== "object" || message === null) {
    return false;
  }

  const msg = message as { type?: string };
  return msg.type === "GET_ADDRESS" || msg.type === "SEND_TO_ADDRESS";
}

export function isWalletResponse(message: unknown): message is WalletResponse {
  if (typeof message !== "object" || message === null) {
    return false;
  }

  const msg = message as { type?: string };
  return (
    msg.type === "ADDRESS_RESPONSE" ||
    msg.type === "SEND_TO_ADDRESS_RESPONSE" ||
    msg.type === "ERROR"
  );
}

export function isWalletNotification(
  message: unknown,
): message is WalletNotification {
  if (typeof message !== "object" || message === null) {
    return false;
  }

  const msg = message as { type?: string };
  return msg.type === "PAYMENT_RECEIVED";
}
