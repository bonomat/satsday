import {GameHistoryItem} from './api';

export interface WebSocketMessage {
    type: 'history' | 'update';
    games?: GameHistoryItem[];
    game?: GameHistoryItem;
}

export type WebSocketCallback = (data: GameHistoryItem | GameHistoryItem[]) => void;
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
        const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || window.location.origin;

        // Convert http/https to ws/wss
        let wsUrl = API_BASE_URL
            .replace(/^https:/, 'wss:')
            .replace(/^http:/, 'ws:');

        // Add /ws endpoint
        wsUrl = wsUrl.endsWith('/') ? `${wsUrl}ws` : `${wsUrl}/ws`;

        return wsUrl;
    }

    private connect(): void {
        if (this.ws?.readyState === WebSocket.OPEN || this.ws?.readyState === WebSocket.CONNECTING) {
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

                    const data = JSON.parse(event.data);

                    if (data.type === 'history' && data.games) {
                        // Initial history load
                        this.cachedHistory = data.games;
                        this.notifySubscribers(data.games);
                    } else {
                        // Real-time update - single game
                        this.notifySubscribers(data);
                    }
                } catch (error) {
                    console.error('Failed to parse WebSocket message:', error);
                }
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
            };

            this.ws.onclose = () => {
                console.log('WebSocket disconnected');
                this.notifyConnectionStatus(false);

                if (this.shouldReconnect) {
                    // Reconnect after delay
                    this.reconnectTimeout = setTimeout(() => {
                        this.connect();
                    }, this.reconnectDelay);
                }
            };
        } catch (error) {
            console.error('Failed to connect WebSocket:', error);
            this.notifyConnectionStatus(false);
        }
    }

    private notifySubscribers(data: GameHistoryItem | GameHistoryItem[]): void {
        this.messageCallbacks.forEach(callback => {
            callback(data);
        });
    }

    private notifyConnectionStatus(connected: boolean): void {
        this.connectionCallbacks.forEach(callback => {
            callback(connected);
        });
    }

    public subscribe(onMessage: WebSocketCallback, onConnectionChange?: ConnectionCallback): () => void {
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

// Create singleton instance
export const gameWebSocketService = new GameWebSocketService();