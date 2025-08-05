const API_BASE_URL = import.meta.env.VITE_API_URL || "http://localhost:12345";

export interface GameHistoryItem {
  id: string;
  amount_sent: string;
  multiplier: number;
  result_number: number;
  target_number: number;
  is_win: boolean;
  payout: string;
  input_tx_id: string;
  output_tx_id: string | null;
  nonce?: string;
  nonce_hash: string;
  timestamp: number;
}

export interface GameHistoryResponse {
  games: GameHistoryItem[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

export async function fetchGameHistory(
  page = 1,
  pageSize = 20,
): Promise<GameHistoryResponse> {
  const response = await fetch(
    `${API_BASE_URL}/games?page=${page}&page_size=${pageSize}`,
  );

  if (!response.ok) {
    throw new Error("Failed to fetch game history");
  }

  return response.json();
}
