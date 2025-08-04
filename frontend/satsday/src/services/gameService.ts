import {type GameData} from '@/types/game';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:12345';

export const gameService = {
  async fetchGameAddresses(): Promise<GameData> {
    try {
      const response = await fetch(`${API_BASE_URL}/game-addresses`);
      if (!response.ok) {
        throw new Error('Failed to fetch game addresses');
      }
      return await response.json();
    } catch (error) {
      console.error('Error fetching game addresses:', error);
      throw error;
    }
  }
};