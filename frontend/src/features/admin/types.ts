export type Category = 'us_market' | 'kospi_plan';

export type Note = {
  id: string | number;
  title: string;
  content: string;
  category: Category | string;
  created_at?: string;
};

export type NewsLink = {
  id: string;
  title: string;
  url: string;
  source?: string | null;
  published_at?: string | null;
  created_at?: string;
};
